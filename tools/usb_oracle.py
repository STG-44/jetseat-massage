# Developer-only x86 packet oracle; requires pefile and unicorn.
# Never opens USB devices or executes a Windows process.
from pathlib import Path
import struct,json,hashlib,sys
import pefile
from unicorn import Uc,UC_ARCH_X86,UC_MODE_32,UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX,UC_X86_REG_ESP,UC_X86_REG_ECX
if len(sys.argv) != 3:
 raise SystemExit('Usage: python3 tools/usb_oracle.py /path/to/uwApi.dll /path/to/vectors.json')
path=Path(sys.argv[1])
if hashlib.sha256(path.read_bytes()).hexdigest() != 'ac6b2b72d8254878741c7bf694ee1f9ed9a97ec8c1909a77a0f490d15bc31b29':
 raise SystemExit('Unknown DLL: addresses and oracle are only valid for the documented SHA-256')
pe=pefile.PE(str(path));base=pe.OPTIONAL_HEADER.ImageBase
u=Uc(UC_ARCH_X86,UC_MODE_32)
u.mem_map(base,(pe.OPTIONAL_HEADER.SizeOfImage+4095)&~4095);u.mem_write(base,pe.get_memory_mapped_image())
u.mem_map(0x20000000,0x2000);u.mem_write(0x20000000,b'\x31\xc0\xc3');u.mem_write(0x20000100,b'\xb8\x01\x00\x00\x00\xc2\x14\x00')
u.mem_map(0x30000000,0x100000);u.mem_map(0x40000000,0x10000)
for d in pe.DIRECTORY_ENTRY_IMPORT:
 for imp in d.imports:
  if imp.name==b'IsDebuggerPresent':u.mem_write(imp.address,struct.pack('<I',0x20000000))
  if imp.name==b'WriteFile':u.mem_write(imp.address,struct.pack('<I',0x20000100))
# Runtime stack-cookie validation is irrelevant to the encoded packet.
u.mem_write(0x10010136,b'\xc3')
def call(addr,param=None,obj=None):
 stack=0x300ff000
 u.mem_write(stack,struct.pack('<I',0x20001000)+(struct.pack('<I',param) if param is not None else b''))
 u.reg_write(UC_X86_REG_ESP,stack)
 if obj is not None:u.reg_write(UC_X86_REG_ECX,obj)
 u.emu_start(addr,0x20001000,count=200000)
 return u.reg_read(UC_X86_REG_EAX)
keys=[call(0x10003970,i) for i in range(8)]
print('key schedule:',[hex(v) for v in keys])
reports=[]
def hook(u,addr,size,data):
 if addr==0x20000100:
  ret,handle,buf,count,written,overlapped=struct.unpack('<6I',u.mem_read(u.reg_read(UC_X86_REG_ESP),24))
  reports.append(bytes(u.mem_read(buf,count)))
  u.mem_write(written,struct.pack('<I',count))
u.hook_add(UC_HOOK_CODE,hook)
fixtures=[]
for motors,salt in [([0]*6,[0,0]),([1,2,3,4,5,6],[0,0]),([64]*6,[0,0]),([255,0,0,0,0,0],[0,0]),([255]*6,[0,0]),([10,20,30,40,50,60],[17,219])]:
 obj=0x40000000;u.mem_write(obj,b'\x00'*0x200);u.mem_write(obj,struct.pack('<I',1));u.mem_write(obj+0x134,struct.pack('<I',9));u.mem_write(obj+0x118,b'\x00'+bytes(motors+salt))
 result=call(0x10003df0,obj=obj)
 assert result==1 and len(reports[-1])==9
 fixtures.append({'motors':motors,'salt':salt,'report':list(reports[-1])});print(fixtures[-1])
Path(sys.argv[2]).write_text(json.dumps({'dll_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'keys':keys,'vectors':fixtures},indent=2))

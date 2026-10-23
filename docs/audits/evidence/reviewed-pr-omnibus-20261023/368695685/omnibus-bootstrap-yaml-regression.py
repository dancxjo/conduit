import contextlib,hashlib,importlib.util,io,json,lzma,pathlib,struct,tempfile,types,unittest.mock as mock,yaml
repo=pathlib.Path('work/open-prs-omnibus');p=repo/'targets/raspberry-pi/deployment/linux/bootstrap.py';spec=importlib.util.spec_from_file_location('bootstrap',p);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
rows=[]
for name in ['true','false','null','on','off','yes','climate-pi']:
 with tempfile.TemporaryDirectory(prefix='6331-yaml-')as tmp:
  root=pathlib.Path(tmp);base=root/'base.xz';image=bytearray(4096);image[510:512]=b'\x55\xaa';image[450]=0x0c;struct.pack_into('<II',image,454,1,7);base.write_bytes(lzma.compress(image));key=root/'key.pub';key.write_text('ssh-ed25519 AAAATEST fixture\n');tools=root/'tools';tools.mkdir()
  for tool in ['mcopy','mtype']:(tools/tool).write_text('fixture')
  module.SOURCE_SHA256=hashlib.sha256(base.read_bytes()).hexdigest();args=types.SimpleNamespace(base=base,output=root/'out.img',ssh_public_key=key,hostname=name,user=name,mtools_dir=tools)
  def readback(command):return (args.output.with_suffix('.bootstrap')/command[-1][2:]).read_bytes()
  with mock.patch.object(module.subprocess,'run'),mock.patch.object(module.subprocess,'check_output',side_effect=readback),contextlib.redirect_stdout(io.StringIO()):module.prepare(args)
  side=args.output.with_suffix('.bootstrap');cloud=yaml.safe_load((side/'user-data').read_text());meta=yaml.safe_load((side/'meta-data').read_text());values=[cloud['hostname'],cloud['user']['name'],cloud['runcmd'][-1][3],cloud['runcmd'][-1][5],meta['local-hostname']];passed=all(type(v)is str and v==name for v in values);rows.append({'name':name,'passed':passed,'values':values,'types':[type(v).__name__ for v in values]})
print(json.dumps({'parser':'PyYAML '+yaml.__version__,'scope':'Actual prepare() with real temporary compressed MBR image and sidecar writes; external ssh-keygen/mtools mocked. No distribution image, SD card or physical boot proof.','cases':rows},indent=2));raise SystemExit(0 if all(r['passed']for r in rows)else 1)

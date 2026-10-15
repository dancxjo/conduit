"""Offline correlation of exact retained CDC product receipts and raw serial files."""
import pathlib,json,re,hashlib,argparse

def correlate(directory):
    directory=pathlib.Path(directory)
    receipt=json.loads((directory/'receipt.json').read_text())
    raw_serial=(directory/'serial.log').read_bytes()
    serial=raw_serial.decode()
    assert receipt['validation']=='passed' and receipt['proof_class']=='freestanding-emulator'
    assert hashlib.sha256(raw_serial).hexdigest()==receipt['serial_sha256']
    def records(prefix):
        return [dict(re.findall(r'(\w+)=(\S+)',line)) for line in serial.splitlines() if line.startswith(prefix)]
    def unique(prefix):
        found=records(prefix);assert len(found)==1,(prefix,len(found));return found[0]
    image=receipt['protocol_image_receipt'];build=receipt['build_record']
    assert build['artifact_role']=='product-host' and build['base_commit']==receipt['product_commit']
    assert image['kernel_sha256']==build['elf_sha256']
    attachment=unique('CONDUIT_USB_PREFLIGHT_ATTACHMENT ')
    assert int(attachment['bar'])>0 and int(attachment['slot'])>0
    assert int(attachment['port'])==1 and int(attachment['epoch'])==1
    preflight=unique('CONDUIT_USB_PREFLIGHT_PLAN ')
    result={'architecture':receipt['architecture'],'source_commit':receipt['product_commit'],'package_sha256':receipt['discovery']['package_sha256'],'image_source':image,'preflight':preflight,'controller_attachment':attachment,'serial_sha256':receipt['serial_sha256']}
    if receipt['discovery']['supported']:
        assert receipt['outcome']=='play-retired'
        plan=unique('CONDUIT_PROTOCOL_PLAN ')
        assert preflight['class_source']==plan['source'] and preflight['class_checked']==plan['checked']
        assert all(image[a]==plan[b] for a,b in [('source_document_id','source'),('checked_plot_id','checked'),('expanded_plot_id','expanded')])
        bases=records('CONDUIT_PROTOCOL_BASE ');resources=records('CONDUIT_PROTOCOL_RESOURCE ')
        assert len(bases)==len(resources)==3
        hosts={b['host'] for b in bases};boots={b['boot'] for b in bases};assert len(hosts)==len(boots)==1
        boot=next(iter(boots));prefix=f"conduitos/protocol/{boot}/usb/{attachment['port']}/{attachment['slot']}/{attachment['epoch']}"
        expected={'/controller':('/provider','/control-dma','machine/usb/control-attachment'),'/endpoint-controller':('/endpoint-provider','/endpoint-dma/1','machine/usb/endpoint-in-attachment'),'/write-controller':('/write-provider','/write-dma/1','machine/usb/endpoint-out-attachment')}
        seen=set()
        for base in bases:
            assert base['plan']==plan['plan'] and base['generation']==attachment['epoch']
            suffix=base['base'].removeprefix(prefix);assert suffix in expected and suffix not in seen;seen.add(suffix)
            provider,pool,kind=expected[suffix];assert base['provider']==prefix+provider
            selected=[r for r in resources if r['placement']==base['placement']];assert len(selected)==1
            resource=selected[0];assert resource['plan']==plan['plan'] and resource['pool']==prefix+pool and resource['class']==kind and resource['units']=='1'
        play=unique('CONDUIT_PROTOCOL_PLAY ');alloc=unique('CONDUIT_PROTOCOL_PLAY_ALLOCATIONS ');stop=unique('CONDUIT_PROTOCOL_PLAY_STOPPED ')
        assert all(alloc[k]==stop[k]==play[k] for k in ['body','plan','play'])
        assert alloc['alloc']==alloc['realloc']=='0'
        assert receipt['decoded_serial']['transmitted']==receipt['decoded_serial']['received']==16
        assert receipt['independent_wire']['actual_transmit']==receipt['independent_wire']['actual_receive']==16
        assert receipt['canonical_discovery']['attachment_epoch']==int(attachment['epoch'])
        result.update(plan=plan,selected_bases=bases,selected_resources=resources,play=play,allocation=alloc,host=next(iter(hosts)),boot=boot)
    else:
        assert receipt['outcome']=='refused' and not receipt['discovery']['class_effects_after_verdict']
        assert not records('CONDUIT_PROTOCOL_BASE ') and not records('CONDUIT_PROTOCOL_PLAY ')
        assert all(receipt['independent_wire'][k]==0 for k in ['source_controls','actual_transmit','actual_receive'])
    return result

if __name__=='__main__':
    parser=argparse.ArgumentParser(description='Offline identity joins for retained normal-product CDC receipts; no new native execution.')
    parser.add_argument('--expected-source',required=True)
    parser.add_argument('--output',type=pathlib.Path,required=True)
    for name in ['aarch64-supported','aarch64-refused','x86_64-supported','x86_64-refused']:
        parser.add_argument('--'+name,type=pathlib.Path,required=True)
    args=parser.parse_args();assert re.fullmatch('[0-9a-f]{40}',args.expected_source)
    items=[]
    for arch in ['aarch64','x86_64']:
        for negative,label in [(False,'supported'),(True,'refused')]:
            item=correlate(getattr(args,arch+'_'+label))
            assert item['architecture']==arch and item['source_commit']==args.expected_source
            item['negative']=negative;items.append(item)
    assert len({x['package_sha256'] for x in items})==1
    positive=[x for x in items if not x['negative']]
    for field in ['source','checked','expanded']:
        assert len({x['plan'][field] for x in positive})==1
    for negative in [False,True]:
        pair=[x for x in items if x['negative']==negative]
        assert len({x['image_source']['root_request_sha256'] for x in pair})==1
    assert len({x['host'] for x in positive})==len({x['boot'] for x in positive})==2
    output={'source':args.expected_source,'proof_class':'normal product freestanding emulator; physical compatibility unverified','correlations':items,'scope':'Controller PCI/BAR facts join attachment port/slot/epoch. Selected Base/provider/resource and Host/Boot join each Plan. Body Plan/Play identities remain distinct. Code ownership enforces the controller baseline binding; text is not cryptographic hardware attestation.'}
    with args.output.open('x') as stream:stream.write(json.dumps(output,indent=2)+'\n')
    print('Same-source cross-architecture normal product correlation PASS')

// Trusted fixed ZIP decoder. Caller supplies a credential-scrubbed command
// adapter; this helper never obtains credentials or executes archive content.
export const PUBLICATION_RECEIPT_ZIP_SCRIPT = `import io,stat,sys,zipfile
raw=sys.stdin.buffer.read(262145)
if len(raw)>262144: raise ValueError('Publication receipt ZIP exceeds bound')
with zipfile.ZipFile(io.BytesIO(raw)) as z:
 entries=z.infolist()
 if len(entries)!=1: raise ValueError('Publication receipt ZIP requires one member')
 e=entries[0]; mode=e.external_attr>>16
 if e.filename!='publication-receipt.json' or e.is_dir() or (mode and stat.S_IFMT(mode) not in (0,stat.S_IFREG)): raise ValueError('Unsafe publication receipt member')
 if e.flag_bits&1 or e.compress_type not in (zipfile.ZIP_STORED,zipfile.ZIP_DEFLATED) or not 0<e.file_size<=65536: raise ValueError('Invalid publication receipt expansion')
 with z.open(e) as f:
  data=f.read(65537)
  if len(data)!=e.file_size or len(data)>65536 or f.read(1): raise ValueError('Publication receipt expansion differs')
 sys.stdout.buffer.write(data)
`;
export function decodePublicationReceiptArchive(archive, command) {
  if (!Buffer.isBuffer(archive) || !archive.length || archive.length > 256 * 1024 || typeof command !== 'function') {
    throw new Error('Explicit bounded publication receipt decoder required');
  }
  const result = command('python3', ['-c', PUBLICATION_RECEIPT_ZIP_SCRIPT], {
    input: archive, encoding: null, timeout: 60000, maxBuffer: 65536,
  });
  if (result.error || result.status !== 0 || !Buffer.isBuffer(result.stdout)
      || !result.stdout.length || result.stdout.length > 65536) throw new Error('Publication receipt ZIP refused');
  return result.stdout;
}

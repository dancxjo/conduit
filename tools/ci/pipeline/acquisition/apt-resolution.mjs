/** APT can label distinct metadata records for the same version as a downgrade. */
export function assertAptResolution(output, installed, architecture, compare) {
  let actions = 0;
  for (const line of output.split('\n')) {
    if (/^(Remv|Purg)\b/.test(line)) throw new Error('APT prerequisite resolution would remove an installed package');
    if (!/^Inst\b/.test(line)) continue;
    const match = /^Inst ([a-z0-9][a-z0-9+.-]*(?::[a-z0-9-]+)?)(?: \[[^\]]+\])? \(([A-Za-z0-9.+:~_-]+)(?: |\))/.exec(line);
    if (!match) throw new Error(`Malformed APT installation action: ${line}`);
    const [, identity, version] = match;
    const [name, arch = architecture] = identity.split(':');
    const previous = installed.get(`${name}:${arch}`) ?? installed.get(`${name}:all`);
    if (previous) {
      try { compare(version, previous); }
      catch (error) { throw new Error(`APT prerequisite resolution would downgrade ${identity} from ${previous} to ${version}: ${error.message}`); }
    }
    actions++;
  }
  if (!actions) throw new Error('APT prerequisite resolution contains no installation actions');
}

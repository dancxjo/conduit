const root = document.querySelector('#product-truth');
try {
  const response = await fetch('./site-publication.json', { cache: 'no-store' });
  if (!response.ok) throw new Error('Publication metadata unavailable');
  const publication = await response.json();
  if (publication.schema !== 'conduit.site-publication/v1' || !/^[a-f0-9]{40}$/.test(publication.sourceCommit)) throw new Error('Unknown publication');
  const paragraph = document.createElement('p');
  paragraph.append('This website and its browser workspace were built and checked from ');
  const source = document.createElement('a');
  source.href = `https://github.com/dancxjo/conduit/commit/${publication.sourceCommit}`;
  source.textContent = publication.sourceCommit.slice(0, 12);
  paragraph.append(source, '.');
  const recordings = document.createElement('p');
  recordings.textContent = 'Field Station Clock was captured with this website build. Three Bodies is a retained October 1 recording with updated page presentation; its screens and model text have not been rerun or replaced with new speech.';
  root.replaceChildren(paragraph, recordings);
} catch {
  root.textContent = 'Publication details are unavailable. The release inventory below remains the place to check published software.';
} finally { root.setAttribute('aria-busy', 'false'); }

"""Bounded ordinary-product F2 chapter; semantic provenance plus raw scanout.

Font-role correctness is CPU evidence, not asserted from the Face observer.
"""
import argparse
import importlib.util
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('product', type=Path)
p.add_argument('source')
p.add_argument('output', type=Path)
p.add_argument('--harness', required=True, type=Path)
p.add_argument('--width', required=True, type=int, choices=[640, 1280])
a = p.parse_args()
spec = importlib.util.spec_from_file_location('panel_harness', a.harness.resolve())
h = importlib.util.module_from_spec(spec)
spec.loader.exec_module(h)
g = h.Guest(a.product.resolve(), a.source, a.output.resolve(), a.width,
            480 if a.width == 640 else 800)
try:
    initial = g.face()
    identities = [(i, prop) for i, prop in enumerate(initial['face']['properties'])
                  if isinstance(prop['value'], dict) and 'Identity' in prop['value']]
    assert identities, 'normal product has no typed technical Identity specimen'
    g.receipt.update(chapter_runner_sha256=h.digest(Path(__file__)),
                     harness_sha256=h.digest(a.harness),
                     typed_identity_properties=identities,
                     role_evidence='CPU provenance-to-Code tests; screenshots inspected separately')
    g.capture('primary')
    before = len(g.records('CONDUIT_CRECHE_CHANGE '))
    g.local('f2')
    pages = g.focus()['pages']
    # Imported harness admits 32 captures; reserve two for primary views.
    assert 1 <= pages <= 30, 'proof capture bound; product admission is separate'
    g.receipt['details_pages'] = pages
    # Ordinary page input only; every page has an acknowledged focus observation.
    for _ in range(g.focus()['page'] - 1):
        g.local('pgup')
    for page in range(1, pages + 1):
        assert g.focus()['page'] == page
        assert g.focus()['pages'] == pages
        g.capture(f'details-page-{page}')
        if page < pages:
            g.local('pgdn')
    assert g.face() == initial, 'Details changed authoritative Face/Show'
    assert len(g.records('CONDUIT_CRECHE_CHANGE ')) == before
    g.local('f2')
    assert g.face() == initial
    assert len(g.records('CONDUIT_CRECHE_CHANGE ')) == before
    g.capture('returned-primary')
    for capture in g.receipt['captures']:
        focus = capture['focus']
        assert focus['face_id'] == initial['face']['identity']
        assert focus['face_revision'] == initial['face']['revision']
        assert focus['show_id'] == initial['show']['show_id']
    g.receipt.update(status='passed', source_effects=0,
                     boundary='normal-product QEMU; no physical/human or pixel-inferred font-role claim')
    g.save()
except BaseException as error:
    g.receipt.update(status='failed', error=repr(error))
    g.save()
    raise
finally:
    g.close()

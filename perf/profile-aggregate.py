#!/usr/bin/env python3
"""Aggregates an xctrace Time Profiler export (xctrace export --xpath /trace-toc/run[1]/data/table[@schema="time-profile"]) for the main thread: self and inclusive time, callers of named functions, verbs vs render split. Information only.
  python3 perf/profile-aggregate.py tp.xml [function-name-fragment ...]"""
import sys, collections
import xml.etree.ElementTree as ET
# resolve id/ref
tree = ET.parse(sys.argv[1]); root = tree.getroot()
ids = {}
def res(e):
    if 'ref' in e.attrib: return ids[e.attrib['ref']]
    if 'id' in e.attrib: ids[e.attrib['id']] = e
    return e
# walk in document order to register ids
for e in root.iter(): 
    if 'id' in e.attrib: ids.setdefault(e.attrib['id'], e)
rows = root.findall('.//row')
tot = 0; selfc = collections.Counter(); incl = collections.Counter(); main = 0
mainstate = collections.Counter()
for r in rows:
    th = res(r.find('thread')); 
    if not th.attrib.get('fmt','').startswith('Main Thread'): continue
    if r.find('weight') is None or r.find('thread') is None: continue
    w = res(r.find('weight')); wt = int(w.text) / 1e6
    st = res(r.find('thread-state')).attrib.get('fmt')
    mainstate[st] += wt
    btn = r.find('tagged-backtrace')
    if btn is None: continue
    bt = res(btn)
    bn_ = bt.find('backtrace')
    frames = res(bn_) if bn_ is not None else None
    fr = []
    for f in (frames if frames is not None else []):
        f = res(f)
        nm = f.attrib.get('name', '?'); 
        b = f.find('binary'); bn = res(b).attrib.get('name', '') if b is not None else ''
        fr.append((nm, bn))
    if not fr: continue
    main += wt
    selfc[fr[0]] += wt
    seen = set()
    for x in fr:
        if x not in seen: incl[x] += wt; seen.add(x)
print('main-thread sampled ms', round(main), dict((k, round(v)) for k, v in mainstate.items()))
print('--- SELF top'); 
for k, v in selfc.most_common(25): print('%7.0f %5.1f%%  %s [%s]' % (v, 100*v/main, k[0][:110], k[1]))
print('--- INCLUSIVE top (whoosh/app frames)')
n = 0
for k, v in incl.most_common(400):
    if k[1] in ('Whoosh', 'Whoosh.debug.dylib', 'WhooshUI') or 'Whoosh' in k[1]:
        print('%7.0f %5.1f%%  %s' % (v, 100*v/main, k[0][:140])); n += 1
        if n >= 45: break

# ---- callers / grouping
import re
def stacks():
    for r in rows:
        th = res(r.find('thread')) if r.find('thread') is not None else None
        if th is None or not th.attrib.get('fmt','').startswith('Main Thread'): continue
        if r.find('weight') is None or r.find('tagged-backtrace') is None: continue
        wt = int(res(r.find('weight')).text)/1e6
        bt = res(r.find('tagged-backtrace')); fr = []
        for f in res(bt.find('backtrace')):
            f = res(f); b = f.find('binary')
            fr.append((f.attrib.get('name','?'), res(b).attrib.get('name','') if b is not None else ''))
        yield wt, fr
want = sys.argv[2:] 
for w in want:
    print('=== callers of', w)
    c = collections.Counter(); tot=0
    for wt, fr in stacks():
        for i, (n, b) in enumerate(fr):
            if w in n:
                tot += wt
                # nearest whoosh caller above it (outward)
                up = next(((n2) for (n2, b2) in fr[i+1:] if 'Whoosh' in b2 and 'thunk' not in n2 and 'partial apply' not in n2 and w not in n2), '?')
                c[up] += wt; break
    print('total', round(tot))
    for k, v in c.most_common(6): print('%6.0f  <- %s' % (v, k[:120]))
# what the main thread does outside verbs: group by outermost non-framework frame class
print('=== split: verb handling vs runloop render')
g = collections.Counter()
for wt, fr in stacks():
    names = [n for n,b in fr]
    if any('ControlVerbs.handle' in n for n in names):
        if any('InteractionProbe.answer' in n for n in names): g['measure verb (spinning runloop)'] += wt
        elif any('AppModel.press' in n for n in names): g['key verb: AppModel.press'] += wt
        else: g['other verbs (rows/uistate/..)'] += wt
    elif any('CA::Transaction' in n or 'CADisplayLink' in n or 'ViewGraph' in n or 'AG::' in n for n in names): g['SwiftUI/CA update outside verbs'] += wt
    else: g['other'] += wt
for k,v in g.most_common(): print('%6.0f  %s' % (v,k))
print('=== outside-verb samples: inclusive by Whoosh frame (no thunks)')
inc = collections.Counter(); self_app = collections.Counter(); T=0
for wt, fr in stacks():
    names=[n for n,b in fr]
    if any('ControlVerbs.handle' in n for n in names): continue
    T+=wt
    seen=set()
    for n,b in fr:
        if 'Whoosh' in b and 'thunk' not in n and 'partial apply' not in n and n not in seen:
            inc[n]+=wt; seen.add(n)
    # innermost whoosh frame (self attribution to app code)
    for n,b in fr:
        if 'Whoosh' in b and 'thunk' not in n and 'partial apply' not in n:
            self_app[n]+=wt; break
print('outside-verb ms', round(T))
for k,v in inc.most_common(40): print('%6.0f %5.1f%%  %s'%(v,100*v/T,k[:130]))
print('--- innermost app frame (where time is spent, app-attributed)')
for k,v in self_app.most_common(15): print('%6.0f %5.1f%%  %s'%(v,100*v/T,k[:130]))
print('=== runtime-generic / debug-cost signature and capture frames')
pat = re.compile(r'Demangle|swift_getTypeByMangledName|swift_getGenericMetadata|MetadataCacheKey|swift_conformsToProtocol|swift_getKeyPath|swift_dynamicCast|swift_getWitnessTable|swift_allocateGenericClassMetadata|instantiateWitnessTable')
cap = re.compile(r'ShotLive|CGWindowListCreateImage|ScreenCaptureKit|renderToImage|ImageRenderer|bitmapImageRepForCachingDisplay|cacheDisplay')
a=b=T2=0
for wt, fr in stacks():
    T2+=wt
    s=' '.join(n for n,_ in fr)
    if pat.search(s): a+=wt
    if cap.search(s): b+=wt
print('main-thread total %d ms; swift-runtime generic/demangle/keypath frames in stack: %d ms (%.1f%%); capture/ShotLive frames: %d ms' % (T2,a,100*a/T2,b))
import subprocess

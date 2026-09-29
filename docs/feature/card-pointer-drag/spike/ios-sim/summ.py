import re,sys,collections
runs=collections.OrderedDict()
for line in open(sys.argv[1]):
    m=re.match(r"\S+ \S+ (\S+) \| (.*)",line.rstrip())
    if m and (len(sys.argv)<3 or m.group(1).startswith(sys.argv[2])): runs.setdefault(m.group(1),[]).append(m.group(2))
for run,L in runs.items():
    # split into gestures
    G=[];cur=None
    for l in L:
        p=l.split(" ",2)
        if len(p)<2: continue
        if p[1].startswith("pointerdown"): cur=[];G.append(cur)
        if cur is not None: cur.append(l)
    for gi,g in enumerate(G or [L]):
        t0=int(g[0].split()[0]); rel=lambda l:int(l.split()[0])-t0
        k=lambda pat:[rel(l) for l in g if re.search(pat,l.split(" ",1)[1] if " " in l else "")]
        sc=[re.search(r"page=(\d+),(\d+) board=(\d+)",l) for l in g if l.split()[1]=="scroll"]
        mxy=max([int(m.group(2)) for m in sc if m],default=0); mxb=max([int(m.group(3)) for m in sc if m],default=0)
        clk=[l for l in g if l.split()[1].startswith("click")]
        cl='SUPPRESSED' if any('PREVENTED' in c for c in clk) else ('yes' if clk else 'no')
        tgt=re.search(r"@(\S+)",g[0]).group(1)
        print(f"{run+('#'+str(gi+1) if len(G)>1 else ''):32} down@{tgt:10} LIFT={k(r'LIFT')} aborted={k(r'aborted')} pcancel={k(r'^pointercancel')} dragstart={k(r'^dragstart')} pageY={mxy} boardX={mxb} tmPrevented={sum(1 for l in g if l.split()[1].startswith('touchmove') and 'PREVENTED' in l)} click={cl} end={k(r'END')}")

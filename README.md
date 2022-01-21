# wordle-solver

Try https://powerlanguage.co.uk/wordle for the original, or https://hellowordl.net/ for one that can be done more than once per day.

```
wfraser@odin [master]% cargo run --release
   Compiling wordle-solve v0.1.0 (/home/wfraser/src/wordle-solve)
    Finished release [optimized] target(s) in 2.08s
     Running `target/release/wordle-solve`
candidates: aback, abaft, abase, abash, abate, abbes, abbey, abbot, abeam, abets, and 5140 more
most unique letters, sorted by letter frequency:
        (irate, 0.417339),
        (orate, 0.413526),
        (raise, 0.405181),
        (arise, 0.405181),
        (atone, 0.40426100000000004),
        (oaten, 0.404261),
        (arose, 0.401368),
        (stare, 0.39924200000000004),
        (aster, 0.39924200000000004),
        (taser, 0.39924200000000004), and 3430 more
Type the guess you made. Prefix each letter with: green=*, yellow=?, gray=!: ?i*r!a!t!e
candidates: brick, brigs, brill, brims, bring, brink, briny, brisk, broil, bruin, and 30 more
most unique letters, sorted by letter frequency:
        (prion, 0.321107),
        (groin, 0.314141),
        (grins, 0.299857),
        (broil, 0.298505),
        (crisp, 0.285667),
        (grind, 0.27635),
        (bruin, 0.274829),
        (cribs, 0.274716),
        (drips, 0.274123),
        (prism, 0.270408), and 18 more
Type the guess you made. Prefix each letter with: green=*, yellow=?, gray=!: !p*r*i!o!n
candidates: brick, brigs, brill, brims, brisk, cribs, crick, drill, frigs, frill, and 6 more
most unique letters, sorted by letter frequency:
        (cribs, 0.274716),
        (grids, 0.267157),
        (brims, 0.259457),
        (brigs, 0.254033),
        (frigs, 0.251434),
        (brisk, 0.24034399999999997),
        (frisk, 0.237745),
        (brick, 0.22838099999999997),
        (grimy, 0.22386999999999999)
Type the guess you made. Prefix each letter with: green=*, yellow=?, gray=!: ?c*r*i?b!s
candidates: brick
most unique letters: brick
Type the guess you made. Prefix each letter with: green=*, yellow=?, gray=!:
```

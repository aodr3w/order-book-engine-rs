do we validate order inputs ??
negative quantities ??

document Command:Server & Command:Simulate
add pagination to get_trade_logs handler

If you need "newest first" efficiently, consider:
A second column with inverted timestamp keys, OR
Using an iterator that supports `seek_to_last/prev` (if ParityDB adds it).


Cap the query limit where you already compute it, but return 400 if someone asks for a silly number (e.g., >10_000) instead of silently clipping, if you want stricter contracts.


If you want zero allocation for symbol comparisons, add Pair::code_str(&self) -> &'static str (match on your two/three supported pairs), then use pair.code_str() everywhere instead of String. That’ll also simplify WS comparisons.


Move to a RWLock 
Add design choices section to README
- locking
- storage (Cursor logic)
- orderbook structure


Add delete order section to README.md
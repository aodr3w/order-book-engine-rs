do we validate order inputs ??
negative quantities ??

document Command:Server & Command:Simulate
add pagination to get_trade_logs handler

If you need "newest first" efficiently, consider:
A second column with inverted timestamp keys, OR
Using an iterator that supports `seek_to_last/prev` (if ParityDB adds it).


Move to a RWLock 
Add design choices section to README
- locking
- storage (Cursor logic)
- orderbook structure
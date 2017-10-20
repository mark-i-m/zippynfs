Latency of directory operations
===============================

Run with AWS client and server.

Benchmark                | Latency
-------------------------|----------
time touch `seq 1 1000`  |  13.5s
time rm `seq 1 1000`     |   8.0s


----------------------

Write 10MiB (see graph)
===========

Time taken to copy 10MiB into the NFS.

Client     | Server      | Writes     | Time (s)
-----------|-------------|------------|-------------
AWS        | AWS         | UNSTABLE   |   1.4
AWS        | AWS         | FILE_SYNC  | 218.5
seclab8    | AWS         | UNSTABLE   | 145.9
seclab8    | AWS         | FILE_SYNC  | 259.7
seclab8    | seclab8     | UNSTABLE   |   0.3
seclab8    | seclab8     | FILE_SYNC  | 446.4


-----------------------

Bandwidth of Write on localhost on 1MiB files (see graph)
=============================================

Num clients | BW per client (MB/s)
------------|---------------
9           | 2.1
8           | 2.2
7           | 2.5
6           | 2.7
5           | 3.1
4           | 3.5
3           | 3.8
2           | 4.6
1           | 7.2


-----------------------

Latency of 10MiB writes with failure before commit
==================================================

Only UNSTABLE writes + Commit.

Client     | Server      | Time (s)
-----------|-------------|-------------
AWS        | AWS         |   5.6
seclab8    | AWS         | 294.7
seclab8    | seclab8     |   3.5

namespace rs zippy

struct ZipTimeVal {
    1: required i64 seconds;
    2: required i64 useconds;
}

struct ZipSattr {
    1: required i16 mode;
    2: required i64 uid;
    3: required i64 gid;
    4: required ZipTimeVal atime;
    5: required ZipTimeVal mtime;
}

enum ZipFtype {
    NFNON = 0,
    NFREG = 1,
    NFDIR = 2,
    NFBLK = 3,
    NFCHR = 4,
    NFLNK = 5
}

struct ZipFattr {
    1: required ZipFtype type;
    2: required i16 mode;
    3: required i64 nlink;
    4: required i64 uid;
    5: required i64 gid;
    6: required i64 size;
    7: required i64 blocksize;
    8: required i64 rdev;
    9: required i64 blocks;
   10: required i64 fsid;
   11: required i64 fid;
   12: required ZipTimeVal atime;
   13: required ZipTimeVal mtime;
   14: required ZipTimeVal ctime;
}

struct ZipFileHandle {
    1: required i64 fid;
}

struct ZipAttrStat {
    1: required ZipFattr attributes
}

struct ZipSattrArgs{
}

struct ZipDirOpArgs{
    1: required ZipFileHandle dir;
    2: required string filename;
}

struct ZipDirOpRes{
    1: required ZipFileHandle file;
    2: required ZipFattr attributes;
}

struct ZipReadArgs{
}

struct ZipReadRes{
}

struct ZipReadDirArgs{
    1: required ZipFileHandle dir;
}

struct ZipDirEntry {
    1: required i64 fid;
    2: required string fname;
}

struct ZipReadDirRes{
    1: required list<ZipDirEntry> entries;
}

struct ZipWriteArgs{
}

struct ZipCreateArgs{
    1: required ZipDirOpArgs where;
    2: required ZipSattr attributes;
}

struct ZipStatFsRes{
}

struct ZipRenameArgs{
}

struct ZipCommitArgs{
}

struct ZipCommitRes{
}

service Zippynfs {
   void null();
   ZipAttrStat getattr(1:ZipFileHandle fhandle);
   ZipAttrStat setattr(1:ZipSattrArgs fsargs);
   ZipDirOpRes lookup(1:ZipDirOpArgs fsargs);
   ZipReadRes read(1:ZipReadArgs fsargs);
   ZipAttrStat write(1:ZipWriteArgs fsargs);
   ZipDirOpRes create(1:ZipCreateArgs fsargs);
   void remove(1:ZipDirOpArgs fsargs);
   void rename(1:ZipRenameArgs fsargs);
   ZipDirOpRes mkdir(1:ZipCreateArgs fsargs);
   void rmdir(1:ZipDirOpArgs fsargs);
   ZipReadDirRes readdir(1:ZipReadDirArgs fsargs);
   ZipStatFsRes statfs(1:ZipFileHandle fhandle);
   ZipCommitRes commit(1:ZipCommitArgs fsargs)
}

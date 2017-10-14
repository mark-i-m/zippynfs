namespace rs zippy

exception ZipException {
    1: required ZipErrorType error;
    2: required string message;
}

enum ZipErrorType {
   NFSERR_NOENT,
   NFSERR_EXIST,
   NFSERR_NOTDIR,
   NFSERR_ISDIR,
   NFSERR_NOTEMPTY,
   NFSERR_STALE,
}

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
    1: required ZipFileHandle file;
    2: required ZipSattr attributes;
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
    1: required ZipFileHandle file;
    2: required i64 offset;
    3: required i64 count;
}

struct ZipReadRes{
    1: required ZipFattr attributes;
    2: required binary data;
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

enum ZipWriteStable {
    UNSTABLE = 0,
    DATA_SYNC = 1,
    FILE_SYNC = 2,
}

struct ZipWriteArgs{
    1: required ZipFileHandle file;
    2: required i64 offset;
    3: required i64 count;
    4: required binary data;
    5: required ZipWriteStable stable;
}

struct ZipWriteRes {
    1: required i64 count;
    2: required ZipWriteStable committed;
    3: required i64 verf;
}

struct ZipCreateArgs{
    1: required ZipDirOpArgs where;
    2: required ZipSattr attributes;
}

struct ZipStatFsRes{
    1: required i64 tsize;
    2: required i64 bsize;
    3: required i64 blocks;
    4: required i64 bfree;
    5: required i64 bavail;
}

struct ZipRenameArgs{
    1: required ZipDirOpArgs old_loc;
    2: required ZipDirOpArgs new_loc;
}

struct ZipCommitArgs{
    1: required ZipFileHandle file;
    2: required i64 count;
    3: required i64 offset;
}

struct ZipCommitRes{
    1: required i64 verf;
}

service Zippynfs {
   void null();
   ZipAttrStat getattr(1:ZipFileHandle fhandle) throws (1: ZipException ex);
   ZipAttrStat setattr(1:ZipSattrArgs fsargs) throws (1: ZipException ex);
   ZipDirOpRes lookup(1:ZipDirOpArgs fsargs) throws (1: ZipException ex);
   ZipReadRes read(1:ZipReadArgs fsargs) throws (1: ZipException ex);
   ZipWriteRes write(1:ZipWriteArgs fsargs) throws (1: ZipException ex);
   ZipDirOpRes create(1:ZipCreateArgs fsargs) throws (1: ZipException ex);
   void remove(1:ZipDirOpArgs fsargs) throws (1: ZipException ex);
   void rename(1:ZipRenameArgs fsargs) throws (1: ZipException ex);
   ZipDirOpRes mkdir(1:ZipCreateArgs fsargs) throws (1: ZipException ex);
   void rmdir(1:ZipDirOpArgs fsargs) throws (1: ZipException ex);
   ZipReadDirRes readdir(1:ZipReadDirArgs fsargs) throws (1: ZipException ex);
   ZipStatFsRes statfs(1:ZipFileHandle fhandle) throws (1: ZipException ex);
   ZipCommitRes commit(1:ZipCommitArgs fsargs) throws (1: ZipException ex);
}

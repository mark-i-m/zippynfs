namespace rs zippy

struct ZipFileHandle {
}

struct ZipAttrStat {
}

struct ZipSattrArgs{
}

struct ZipDirOpArgs{
}

struct ZipDirOpRes{
}

struct ZipReadArgs{
}

struct ZipReadRes{
}

struct ZipReadDirArgs{
}

struct ZipReadDirRes{
}

struct ZipWriteArgs{
}

struct ZipCreateArgs{
}

struct ZipStat{
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
   ZipStat remove(1:ZipDirOpArgs fsargs);
   ZipStat rename(1:ZipRenameArgs fsargs);
   ZipDirOpRes mkdir(1:ZipCreateArgs fsargs);
   ZipStat rmdir(1:ZipDirOpArgs fsargs);
   ZipReadDirRes readdir(1:ZipReadArgs fsargs);
   ZipStatFsRes statfs(1:ZipFileHandle fhandle);
   ZipCommitRes commit(1:ZipCommitArgs fsargs)
}

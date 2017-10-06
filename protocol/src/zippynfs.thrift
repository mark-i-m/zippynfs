namespace rs zippy

struct FileHandle {
}

struct AttrStat {
}

struct SattrArgs{
}

struct DirOpArgs{
}

struct DirOpRes{
}

struct ReadArgs{
}

struct ReadRes{
}

struct ReadDirArgs{
}

struct ReadDirRes{
}

struct WriteArgs{
}

struct CreateArgs{
}

struct Stat{
}

struct StatFsRes{
}

struct RenameArgs{
}

struct CommitArgs{
}

struct CommitRes{
}

service Zippynfs {

   void null();
   AttrStat getattr(1:FileHandle fhandle);
   AttrStat setattr(1:SattrArgs setargs);
   DirOpRes lookup(1:DirOpArgs dirargs);
   ReadRes read(1:ReadArgs rdargs);
   AttrStat write(1:WriteArgs wrargs);
   DirOpRes create(1:CreateArgs crtargs);
   Stat remove(1:DirOpArgs rmfargs);
   Stat rename(1:RenameArgs mvargs);
   DirOpRes mkdir(1:CreateArgs crtdargs);
   Stat rmdir(1:DirOpArgs rmdargs);
   ReadDirRes readdir(1:ReadArgs rddargs);
   StatFsRes statfs(1:FileHandle fhndl);
   CommitRes commit(1:CommitArgs cmtargs)
}

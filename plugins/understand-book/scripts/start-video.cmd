@echo off
setlocal
set "VIDEO_BUILD_BIN=%UNDERSTAND_BOOK_BUILD_EXE%"
if defined VIDEO_BUILD_BIN goto run
for /f "tokens=2,*" %%A in ('reg.exe query "HKCU\Software\UnderstandBook" /v InstallDir 2^>nul') do (
  if /i "%%A"=="REG_SZ" set "VIDEO_BUILD_BIN=%%B\understand-book-build.exe"
)
:run
if not defined VIDEO_BUILD_BIN goto unavailable
if not exist "%VIDEO_BUILD_BIN%" goto unavailable
"%VIDEO_BUILD_BIN%" %*
exit /b %ERRORLEVEL%
:unavailable
1>&2 echo Install Understand Book Setup or set UNDERSTAND_BOOK_BUILD_EXE, then run start-video.cmd video-doctor.
exit /b 2

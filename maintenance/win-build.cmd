@echo off
setlocal ENABLEDELAYEDEXPANSION

set project=wolfsmuehle
set exe=%project%.exe
set version_crate_subdir=.

set GTK_DIR=C:\gtk-build\gtk\x64\release
set GTK_BIN=%GTK_DIR%\bin

set PATH=%PATH%;C:\WINDOWS;C:\WINDOWS\SYSTEM32
set PATH=%GTK_BIN%;%PATH%
set PATH=%PATH%;%ProgramFiles%/7-Zip

cd ..
if ERRORLEVEL 1 goto error_basedir

call :detect_version
if "%PROCESSOR_ARCHITECTURE%" == "x86" (
    set winprefix=win32
) else (
    set winprefix=win64
)
set distdir=%project%-%winprefix%-%version%
set sfxfile=%project%-%winprefix%-%version%.package.exe

call :prepare_env
call :build
call :copy_files
call :gen_wrapper
call :archive

echo Successfully built.
pause
exit /B 0

:detect_version
    pushd %version_crate_subdir%
    if not exist Cargo.lock (
        cargo update
        if ERRORLEVEL 1 goto error_version
    )
    for /f "tokens=2 delims=#" %%a in ('cargo pkgid') do set version=%%a
    for /f "tokens=2 delims=:" %%a in ("%version%") do set version=%%a
    if ERRORLEVEL 1 goto error_version
    echo Detected version: %version%
    popd
    exit /B 0

:prepare_env
    rd /S /Q %distdir% 2>NUL
    del %sfxfile% 2>NUL
    timeout \T 2 \NOBREAK 2>NUL
    mkdir %distdir%
    if ERRORLEVEL 1 goto error_prep
    exit /B 0

:build
    cargo clean
    if ERRORLEVEL 1 goto error_build
    cargo update
    if ERRORLEVEL 1 goto error_build
    cargo build --release
    if ERRORLEVEL 1 goto error_build
    exit /B 0

:copy_files
    mkdir %distdir%\bin
    if ERRORLEVEL 1 goto error_copy
    copy target\release\%exe% %distdir%\bin\
    if ERRORLEVEL 1 goto error_copy
    xcopy /E /I %GTK_DIR% %distdir%\gtk
    if ERRORLEVEL 1 goto error_copy
    rd /S /Q %distdir%\gtk\include
    if ERRORLEVEL 1 goto error_copy
    exit /B 0

:gen_wrapper
    set wrapper=%distdir%\wolfsmuehle.cmd
    echo @echo off > %wrapper%
    echo set PATH=gtk\bin;%%PATH%% >> %wrapper%
    echo start bin\%exe% %%1 %%2 %%3 %%4 %%5 %%6 %%7 %%8 %%9 >> %wrapper%
    if ERRORLEVEL 1 goto error_wrapper
    exit /B 0

:archive
    7z a -mx=9 -sfx7z.sfx %sfxfile% %distdir%
    if ERRORLEVEL 1 goto error_7z
    exit /B 0

:error_basedir
    echo FAILED to CD to base directory.
    goto error
:error_version
    echo FAILED to detect version.
    goto error
:error_prep
    echo FAILED to prepare environment.
    goto error
:error_build
    echo FAILED to build.
    goto error
:error_copy
    echo FAILED to copy files.
    goto error
:error_wrapper
    echo FAILED to create wrapper.
    goto error
:error_7z
    echo FAILED to compress archive.
    goto error
:error
    pause
    exit 1

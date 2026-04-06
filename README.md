# Video Finder

A personal application for finding videos in a database.

## About

A Rust application that's using [Slint](https://slint.rs/) for the user interface.

## Compiling

### Building on an Android tablet directly

First, follow the steps at https://github.com/dfaure/rust-android-hello-world
In addition, this project requires:
```
pkg install ndk-multilib
```
ndk-multilib is a Termux package that provides pre-compiled C/C++ system libraries
(libc, libm, libz, etc.) for all Android target architectures. It's needed when
compiling native code  that links against standard C libraries. In this project's case,
*rusqlite* with the *bundled* feature compiles SQLite from C source, so it needs those headers and libraries.

## Usage

Only meaningful to the author and his wife, move along ;-)

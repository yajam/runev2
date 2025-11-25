# Offscreen rendering with CEF and Metal

This project is a POC to combine CEF offscreen rendering with Metal API on MacOS. It's based on the `cefsimple` test found in the CEF distribution. This can be a base for a CEF based HUD on top of a Metalkit layer for e.g. a game.

The app creates a single window with an ever changing background color and a CEF texture on top of it. Transparent areas of the browser window uncover
the animated background.

## About CEF in general
Check the documentation at https://bitbucket.org/chromiumembedded/cef/wiki/GeneralUsage


## Setting it up
You need to manually download and extract the CEF binaries and set it up so that it creates an xcode project file that is referred by the `testapp.xcodeproj`. 
(It was tested to work with `cef_binary_86.0.21+g6a2c8e7+chromium-86.0.4240.183_macosx64.tar.bz2`.)

1. Download CEF binaries

- https://cef-builds.spotifycdn.com/index.html#macosx64
- macos 64-bit
- select "standard distribution"

2. Extract into the `cef` folder:

```
> ls -la cef
total 200
drwxr-xr-x  15 encse  staff    480 Nov 13 11:03 .
drwxr-xr-x  12 encse  staff    384 Nov 13 13:42 ..
-rw-r--r--@  1 encse  staff   6148 Nov 13 13:03 .DS_Store
-rw-r--r--   1 encse  staff   8131 Nov  4 07:35 CMakeLists.txt
drwxr-xr-x   4 encse  staff    128 Nov 13 10:53 Debug
-rw-r--r--   1 encse  staff   1662 Nov  4 05:17 LICENSE.txt
-rw-r--r--   1 encse  staff   7637 Nov  4 07:35 README.txt
drwxr-xr-x   4 encse  staff    128 Nov  4 07:35 Release
-rw-r--r--   1 encse  staff  42428 Nov  4 05:17 cef_paths.gypi
-rw-r--r--   1 encse  staff  27542 Nov  4 05:17 cef_paths2.gypi
drwxr-xr-x   5 encse  staff    160 Nov  4 07:35 cmake
drwxr-xr-x  89 encse  staff   2848 Nov  4 07:36 include
drwxr-xr-x  13 encse  staff    416 Nov  4 07:35 libcef_dll
drwxr-xr-x   7 encse  staff    224 Nov  4 07:35 tests
```

3. Follow the instructions in `cef/README.txt` and `cef/CMakeLists.txt` to create a `build` folder inside the `cef` directory with `cef.xcodeproj`.

4. Open `testapp.xcodeproj` in xcode, select the `testapp` target at run the project.

Enjoy your psychedelic space trip. (The real thing is much smoother than this gif.)

![Spaceship](./spaceship.gif "Spaceship")

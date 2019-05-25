mystcl
======

`mystcl` is a basic Tcl/Tk interface for Python.

Its existance is meant as a tkinter replacement for [teek](https://github.com/Akuli/teek).

Dependencies
------------

Installing `mystcl` via PyPI has no dependencies, as the `Dockerfile`
includes the required dependenceis in the wheel itself.

Installation
------------

Every `master` tag is uploaded to PyPI automatically via Travis, so you can
install `mystcl` simply by running:

```sh
$ python3 -m pip install --user --upgrade mystcl
```

Compilation
-----------

Compilation has dependencies on `rust` nightly,  tcl8.6 and tk8.6. You can
compile just via `cargo buidl --release`. You'll get a `libmystcl.so` in
`target/release/` which you can rename to `mystcl.so` and use.

# PYTHON FFI EXAMPLE

This demo runs a full 3D particle engine without compiling a single binary. Kain handles the heavy lifting—physics, math, and object management. It uses the Python FFI to pipe draw commands directly to tkinter.

It proves you can use Kain for high-performance logic while leveraging Python libraries for immediate visualization.

Run it with this simple command! 
kain tests\visual_demo.kn -t run

You can also use the --watch flag which puts the compiler in "monitor mode." It tracks your source file and automatically re-runs your code the moment you save changes.

Run using the interpreter subcommand with watch mode enabled
kain run tests\visual_demo.kn --watch

Instant feedback loop. Save file → See result.
![Kain 3D Python FFI](kain3d_pythonFFI.png)

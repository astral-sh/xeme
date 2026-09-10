# Full distribution glibc baseline failure and link correction

[Run 34439081013](https://github.com/astral-sh/oriole/actions/runs/34439081013)
completed the validator build and the full CPython 3.12.13 distribution build.
The distribution validator then rejected a glibc 2.18 requirement in both the
interpreter and shared libpython. XML and installed-identity checks did not run.
The archive identity and complete compressed build logs are retained here.

The undefined `__cxa_thread_atexit_impl` reference is weak, but the linked ELF
version requirement is strong. PBS's Jessie sysroot provides this optional hook
although the distribution targets glibc 2.17. Rust's standard library checks the
hook for null and otherwise uses its existing pthread-key destructor path.
[The upstream Rust issue](https://github.com/rust-lang/rust/issues/57497) describes
the same symbol-version problem.

The opt-in overlay now wraps this one undefined symbol without defining a wrapper.
Its unversioned weak address remains null, selecting Rust's existing fallback.
The bundle preparer requires the original reference to be weak. The overlay does
not change the runtime implementation of glibc's hook or relax PBS validation.
This is limited to the pinned native Linux CPython overlay; it is not a general
link recipe for arbitrary C++ libraries or unloadable Rust plugins.

A real Rust thread-local Drop probe runs on 32 C-created threads in native,
static-fallback, and shared-fallback executables. Every destructor runs exactly
once, and fallback ELF symbols retain only the unversioned undefined weak wrapper.
An earlier hidden-absolute-zero linker-script prototype crashed under PIE and was
rejected; no zero-symbol definition or linker script is included in this change.
The actual full-distribution validator remains required. CI additionally runs the
installed Oriole Python on pinned CentOS 7/glibc 2.17 with 1,024 threaded parses and
all six XML standard-library suites. That full follow-up result is pending.

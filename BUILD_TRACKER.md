# Build Tracker

This marker file tracks automated rebuild cycles pushed to `salom600/engin`.

Each push to `main` triggers the GitHub Actions workflow at
`.github/workflows/build.yml`, which:

1. Builds release binaries for Windows and Linux.
2. Uploads them as workflow artifacts.
3. Refreshes the rolling `build-latest` prerelease with both zips.

If any platform fails, the failure is analyzed, the source is patched, and
a new commit is pushed — the cycle repeats until both OS builds are green.

# Repository Guidelines

## Project Status

This workspace is currently empty. No application code, dependency manifests, build scripts, tests, or Git history are present. Treat the conventions below as starting guidance; update this document when the project establishes its stack and workflows.

## Project Structure & Module Organization

When introducing code, organize files by responsibility. Prefer `src/` for application code, `tests/` for automated tests, `assets/` for static resources, and `docs/` for supporting documentation unless the chosen framework prescribes another layout. Create directories only when needed. Add a root `README.md` describing the project's purpose and setup.

## Build, Test, and Development Commands

No build, test, or development commands are configured yet. The first implementation should provide reproducible commands through the selected ecosystem's standard tooling and document them in `README.md`. Include dependency installation, local startup, production builds when applicable, and test execution. Do not assume commands such as `npm test` exist before adding the corresponding configuration.

## Coding Style & Naming Conventions

Follow the chosen language's standard formatter and naming conventions. Configure formatting and linting alongside the first source files, including indentation rules. Use descriptive names, keep modules focused, and avoid unrelated formatting changes. Follow established patterns once they exist.

## Testing Guidelines

No testing framework or coverage threshold is configured. Select a framework appropriate to the stack and document test discovery and execution. Use descriptive test names that state expected behavior. Cover new behavior and add regression tests for bug fixes.

## Commit & Pull Request Guidelines

There is no Git history from which to infer commit conventions. Use concise, imperative subjects such as `Add daily word model`. Keep commits focused. Pull requests should explain the change, list verification performed, link relevant issues, and include screenshots for visible interface changes.

## Security & Configuration

Never commit credentials or secrets. When configuration is introduced, provide placeholder examples and ignore local secrets and generated artifacts.

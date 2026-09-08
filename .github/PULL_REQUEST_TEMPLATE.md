## What this changes

<!-- One paragraph. What is different after this lands. -->

## Why

<!-- The reason it should. A defect this fixes, a capability the consumer
     needs, a decision recorded elsewhere. -->

## How it was checked

<!-- `make pre-commit` at minimum. If the change touches the Apple build,
     say which slices you built and on what. If it touches the binding's
     shape, say what you generated the Swift against. -->

- [ ] `make pre-commit` passes
- [ ] The generated Swift still has the shape ggchat's seam expects
- [ ] No credential, ticket or token appears in any new log line or error

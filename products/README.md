# Product boundary

`products/conduit` is the repository's sole product/distribution boundary. It
owns the installed `conduit` command and packages the supported entrances.

Human encounters such as Workspace, Tour, Birth, and Patchbay are not separate
products or runtimes. Their meaning lives with resident Plots and semantic
owners; browser realization lives under `targets/browser`, and ConduitOS
realization lives under `targets/conduitos`.

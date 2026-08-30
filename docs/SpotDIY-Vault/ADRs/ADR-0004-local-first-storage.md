# ADR-0004: local-first storage

Status: accepted

SQLite WAL, local files, and optional local caches are authoritative for user data. Standard mode targets `%LOCALAPPDATA%\SpotDIY`; portable mode is selected deterministically and keeps data beside the executable. Secure credentials are outside SQLite.

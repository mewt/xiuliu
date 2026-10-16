# Vocabulary sources and attribution

The original 30 example-based lessons are in `words.json`.

`hsk.json` is adapted from **Complete HSK Vocabulary**, maintained by
Yanis Zafirópulos (Dr.Kameleon) and contributors:
https://github.com/drkameleon/complete-hsk-vocabulary

A selected 3,312-entry source snapshot was retrieved on 2026-10-16, including
the full 506-entry new-level-1 list and a selection from higher levels. It is not
the complete upstream inventory; the update command retrieves that inventory.
Changes: selected one reading per
simplified headword (preferring a common non-surname reading), retained up to
three definitions, retained HSK level and frequency, and converted the fields
into the app's JSON format. The source's `new-*` levels are preferred, with
`old-*` levels used when no `new-*` level is present. This is a vocabulary course,
not a claim of conformity with any particular current exam syllabus.

The snapshot entry for `听` uses the common reading `tīng` (to listen), with
traditional form `聽`, matching the curated lesson rather than an archaic reading.

The upstream project is distributed under the MIT license, reproduced in
`HSK-LICENSE.txt`. Its README attributes the dictionary definitions to
**CC-CEDICT**, maintained by MDBG and the CC-CEDICT community:
https://www.mdbg.net/chinese/dictionary?page=cc-cedict

CC-CEDICT is available under Creative Commons Attribution-ShareAlike 4.0:
https://creativecommons.org/licenses/by-sa/4.0/

Retain this attribution and the applicable share-alike terms for dictionary
material when redistributing adapted vocabulary data. The application's code
and third-party vocabulary have separate licensing terms. Other upstream
contributors and sources are listed in the Complete HSK Vocabulary README.

The `update` command fetches the upstream `complete.json` over HTTPS. It applies
the same field conversion and saves the resulting vocabulary locally. It does
not generate definitions or example sentences with AI.

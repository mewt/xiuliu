# Vocabulary sources and attribution

The original 30 example-based entries are in `words.json`. The explicit 15-lesson
sequence is stored in `course.json`. It follows the vocabulary order and lesson
grouping of the privately consulted 2014 edition of *HSK Standard Course 1*
(ISBN 978-7-5619-3709-9), edited by Jiang Liping and published by Beijing Language
and Culture University Press. No textbook pages, dialogues, exercises, or example
sentences are distributed with the app. `textbook-words.json` contains independently
written basic entries needed by that sequence.

`hsk.json` is adapted from **Complete HSK Vocabulary**, maintained by
Yanis Zafirópulos (Dr.Kameleon) and contributors:
https://github.com/drkameleon/complete-hsk-vocabulary

A selected 3,312-entry source snapshot was retrieved on 2026-10-16, including
the full 506-entry new-level-1 list and a selection from higher levels. The active
course selects only entries used by its HSK Standard Course 1 lesson progression;
other entries remain source data but are not loaded.
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

The course is bundled and works offline. Example sentences in `words.json` and
`examples.json` are separately curated; the dictionary source does not provide
them.

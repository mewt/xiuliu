#!/usr/bin/env python3
"""Generate reviewable HSK 1 examples with Xiaomi MiMo; does not modify app data."""
import csv
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"
OUT_JSON = ASSETS / "hsk1-examples-mimo-v2.6-pro.json"
OUT_CSV = ASSETS / "hsk1-examples-mimo-v2.6-pro.csv"
MODEL = "mimo-v2.6-pro"
BATCH_SIZE = 10


def settings():
    values = {}
    env_path = ROOT / ".env"
    if env_path.exists():
        for raw in env_path.read_text(encoding="utf-8").splitlines():
            line = raw.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, value = line.split("=", 1)
            values[key.strip()] = value.strip().strip("\"'")
    key = os.environ.get("MIMO_API_KEY", values.get("MIMO_API_KEY", ""))
    base = os.environ.get("MIMO_BASE_URL", values.get("MIMO_BASE_URL", "")).rstrip("/")
    if not key or not base.startswith("https://"):
        raise SystemExit("MIMO_API_KEY and an HTTPS MIMO_BASE_URL are required")
    return key, base


def load_words():
    hsk = json.loads((ASSETS / "hsk.json").read_text(encoding="utf-8"))
    curated = json.loads((ASSETS / "words.json").read_text(encoding="utf-8"))
    curated_by_word = {word["simplified"]: word for word in curated}
    words = [dict(word) for word in hsk if word["level"] == 1]
    # Apply the app's textbook corrections (right reading/sense) before generating.
    textbook = {w["simplified"]: w for w in
                json.loads((ASSETS / "textbook-words.json").read_text(encoding="utf-8"))}
    index = {w["simplified"]: i for i, w in enumerate(words)}
    for simplified, fix in textbook.items():
        if simplified in index:
            words[index[simplified]].update(
                {k: fix[k] for k in ("traditional", "pinyin", "meaning") if k in fix})
        else:
            words.append(dict(fix))
    for word in words:
        if word["simplified"] in curated_by_word:
            word.update({
                key: curated_by_word[word["simplified"]].get(key, "")
                for key in ("example", "example_traditional", "example_pinyin", "translation")
            })
            word["example_source"] = "curated"
            word["review_status"] = "needs_review"
    return words


def existing_rows():
    if not OUT_JSON.exists():
        return {}
    return {row["simplified"]: row for row in json.loads(OUT_JSON.read_text(encoding="utf-8"))}


TONE_MARKS = "āáǎàēéěèīíǐìōóǒòūúǔùǖǘǚǜĀÁǍÀ"
CJK = lambda s: [c for c in s if "一" <= c <= "鿿"]
_TONELESS = str.maketrans(
    "āáǎàēéěèīíǐìōóǒòūúǔùǖǘǚǜ", "aaaaeeeeiiiioooouuuuuuuu")


def toneless(pinyin):
    """Lowercase, drop tone marks and spaces, so readings can be compared."""
    return pinyin.lower().translate(_TONELESS).replace("ü", "u").replace(" ", "")


def quality_issues(word, example):
    """Soft quality checks. A non-empty list means the row still needs human review."""
    issues = []
    simp, pinyin, trans = example["example"], example["example_pinyin"], example["translation"]
    trad = example["example_traditional"]
    if word["traditional"] not in trad:
        issues.append("traditional headword missing")
    if len(CJK(simp)) > 14:
        issues.append("sentence too long for a beginner")
    if len(CJK(simp)) < 2:
        issues.append("sentence too short")
    if simp[-1] not in "。！？":
        issues.append("no sentence-ending punctuation")
    if not any(mark in pinyin for mark in TONE_MARKS):
        issues.append("pinyin has no tone marks")
    if any(ch.isdigit() for ch in pinyin):
        issues.append("pinyin uses tone numbers, not marks")
    if toneless(word["pinyin"]) not in toneless(pinyin):
        issues.append("headword reading absent from pinyin (wrong sense/homophone?)")
    if not any(ch.isalpha() and ch.isascii() for ch in trans):
        issues.append("translation is not English")
    if simp.count(word["simplified"]) > 2:
        issues.append("headword repeated unnaturally")
    return issues


def request_batch(key, base, words):
    items = [{"simplified": w["simplified"], "traditional": w["traditional"],
              "pinyin": w["pinyin"], "meaning": w["meaning"]} for w in words]
    prompt = f"""You write example sentences for an HSK 1 Mandarin learning app. For every vocabulary item below, write ONE original, natural beginner sentence.

Hard rules for each item:
- Use the exact target headword (the `simplified` value) in the simplified sentence, and its supplied `traditional` form in the traditional sentence. Same meaning, both scripts.
- Use the headword in the sense given by `meaning` and the reading given by `pinyin` — never another sense or a homophone. The sentence must make real, literal sense (e.g. 读 dú means "to read"; do not confuse it with 逗 dòu "comma").
- Keep it short and HSK 1: 3 to 10 characters, and use only common HSK 1 vocabulary around the headword. No rare words, idioms, proper nouns (unless the headword itself is one), or textbook quotations.
- End every sentence with 。, ！, or ？.
- `example_pinyin`: Hanyu Pinyin with TONE MARKS (never tone numbers), matching the sentence exactly. Use sentence case: capitalize ONLY the first letter of the sentence and proper nouns (e.g. Běijīng); every other syllable is lowercase. Separate words with spaces and join a word's own syllables (e.g. péngyou, zhuōzi).
- `translation`: one faithful, natural English sentence.

Return JSON only: an object with key "examples" whose value is an array in the SAME ORDER as the input. Each item must contain exactly: simplified, example, example_traditional, example_pinyin, translation.
Input: {json.dumps(items, ensure_ascii=False)}"""
    payload = {"model": MODEL, "messages": [{"role": "user", "content": prompt}],
               "response_format": {"type": "json_object"}, "temperature": 0.2,
               "max_completion_tokens": 5000}
    request = urllib.request.Request(base + "/chat/completions",
        data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        headers={"Authorization": "Bearer " + key, "Content-Type": "application/json"})
    with urllib.request.urlopen(request, timeout=180) as response:
        body = json.load(response)
    result = json.loads(body["choices"][0]["message"]["content"])
    examples = result["examples"]
    if len(examples) != len(words):
        raise ValueError(f"expected {len(words)} examples, received {len(examples)}")
    output = []
    for word, example in zip(words, examples):
        if example.get("simplified") != word["simplified"]:
            raise ValueError(f"out-of-order response for {word['simplified']}")
        required = ("example", "example_traditional", "example_pinyin", "translation")
        if not all(isinstance(example.get(field), str) and example[field].strip() for field in required):
            raise ValueError(f"incomplete response for {word['simplified']}")
        example = {field: example[field].strip() for field in required}
        # Hard requirement: the word must appear in its own simplified example, or retry.
        if word["simplified"] not in example["example"]:
            raise ValueError(f"target absent from simplified example: {word['simplified']}")
        row = dict(word)
        row.update(example)
        row["example_source"] = MODEL
        issues = quality_issues(word, example)
        row["review_status"] = "needs_review" if issues else "auto_ok"
        if issues:
            row["review_notes"] = "; ".join(issues)
        output.append(row)
    return output




def save(words, completed):
    rows = [completed[w["simplified"]] for w in words if w["simplified"] in completed]
    OUT_JSON.write_text(json.dumps(rows, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    fields = ["simplified", "traditional", "pinyin", "meaning", "level", "frequency",
              "example", "example_traditional", "example_pinyin", "translation",
              "example_source", "review_status", "review_notes"]
    with OUT_CSV.open("w", encoding="utf-8", newline="") as file:
        writer = csv.DictWriter(file, fieldnames=fields, extrasaction="ignore", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def main():
    refresh = "--refresh" in sys.argv[1:]
    key, base = settings()
    words = load_words()
    # --refresh regenerates every model sentence with the current prompt; curated
    # (words.json) sentences are always kept as-is.
    completed = {} if refresh else existing_rows()
    for word in words:
        if word.get("example"):
            completed.setdefault(word["simplified"], word)
    save(words, completed)
    pending = [word for word in words if word["simplified"] not in completed]
    total_batches = (len(pending) + BATCH_SIZE - 1) // BATCH_SIZE
    for offset in range(0, len(pending), BATCH_SIZE):
        batch = pending[offset:offset + BATCH_SIZE]
        number = offset // BATCH_SIZE + 1
        for attempt in range(1, 4):
            try:
                generated = request_batch(key, base, batch)
                for row in generated:
                    completed[row["simplified"]] = row
                save(words, completed)
                print(f"batch {number}/{total_batches}: {len(completed)}/{len(words)} saved", flush=True)
                break
            except (urllib.error.URLError, ValueError, KeyError, json.JSONDecodeError) as error:
                if attempt == 3:
                    raise
                print(f"batch {number} attempt {attempt} failed: {error}; retrying", flush=True)
                time.sleep(2 ** attempt)
    print(f"Complete: {OUT_JSON}\nReview: {OUT_CSV}")


if __name__ == "__main__":
    main()

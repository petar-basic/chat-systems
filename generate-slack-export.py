#!/usr/bin/env python3
import argparse
import bisect
import datetime as dt
import itertools
import json
import math
import random
import zipfile

FIRST_NAMES = [
    "Ana", "Marko", "Jelena", "Nikola", "Milica", "Stefan", "Ivana", "Luka", "Tamara", "Petar",
    "Sara", "Filip", "Maja", "Vuk", "Teodora", "Lazar", "Katarina", "Andrej", "Mina", "Ognjen",
    "Nina", "Dusan", "Jovana", "Aleksa", "Marija", "Uros", "Dragana", "Nemanja", "Sofija", "Bojan",
    "Iva", "Strahinja", "Lena", "Milos", "Tijana", "Vladimir", "Hana", "Igor", "Una", "Zoran",
]
LAST_NAMES = [
    "Petrovic", "Jovanovic", "Nikolic", "Markovic", "Djordjevic", "Stojanovic", "Ilic", "Pavlovic",
    "Milosevic", "Stankovic", "Lukic", "Kostic", "Popovic", "Zivkovic", "Todorovic", "Ristic",
]
COMMON_WORDS = [
    "the", "to", "and", "a", "i", "it", "is", "for", "on", "we", "that", "this", "in", "of", "you",
    "can", "be", "with", "just", "so", "will", "have", "do", "not", "but", "are", "should", "if",
    "deploy", "review", "meeting", "build", "fix", "test", "release", "today", "tomorrow", "ticket",
    "merge", "branch", "issue", "customer", "update", "check", "staging", "production", "error",
    "please", "thanks", "looks", "good", "done", "need", "think", "maybe", "still", "now", "after",
    "before", "here", "there", "what", "why", "when", "how", "who", "yes", "no", "ok", "sure",
    "query", "database", "latency", "dashboard", "alert", "incident", "rollback", "pipeline",
    "design", "spec", "draft", "invoice", "contract", "budget", "roadmap", "sprint", "standup",
]
SYLLABLES = [
    "ka", "lo", "mi", "ne", "ru", "ta", "vo", "zi", "pre", "stra", "gon", "del", "fin", "sar",
    "mon", "tel", "qui", "bre", "dor", "lun", "ves", "pha", "tro", "kel", "nor", "sil",
]
EMOJI = ["thumbsup", "eyes", "white_check_mark", "tada", "heart", "joy", "rocket", "pray", "fire", "+1"]
MIMETYPES = [
    ("report.pdf", "application/pdf"),
    ("screenshot.png", "image/png"),
    ("notes.txt", "text/plain"),
    ("data.csv", "text/csv"),
]


def parse_args():
    parser = argparse.ArgumentParser(
        description=(
            "Writes a synthetic Slack export ZIP shaped like the real thing, for measuring "
            "the importer, search and history at migration scale. The same seed always "
            "produces the same archive."
        )
    )
    parser.add_argument("--out", required=True, help="path of the ZIP to write")
    parser.add_argument("--messages", type=int, default=500_000)
    parser.add_argument("--channels", type=int, default=200, help="public plus private channels")
    parser.add_argument("--private-share", type=float, default=0.1)
    parser.add_argument("--users", type=int, default=40)
    parser.add_argument("--dms", type=int, default=30)
    parser.add_argument("--group-dms", type=int, default=10)
    parser.add_argument("--days", type=int, default=730)
    parser.add_argument("--end", default="2026-09-30", help="last day of history, YYYY-MM-DD")
    parser.add_argument("--seed", type=int, default=47)
    parser.add_argument("--thread-share", type=float, default=0.12)
    parser.add_argument("--reaction-share", type=float, default=0.12)
    parser.add_argument("--file-share", type=float, default=0.02)
    parser.add_argument("--mention-share", type=float, default=0.05)
    return parser.parse_args()


class Vocabulary:
    def __init__(self, rng, size=6000):
        invented = set()
        while len(invented) < size - len(COMMON_WORDS):
            word = "".join(rng.choice(SYLLABLES) for _ in range(rng.randint(2, 4)))
            if word not in COMMON_WORDS:
                invented.add(word)
        tail = sorted(invented)
        rng.shuffle(tail)
        self.words = COMMON_WORDS + tail
        weights = [1 / (rank + 1) ** 1.05 for rank in range(len(self.words))]
        self.cumulative = list(itertools.accumulate(weights))

    def word(self, rng):
        point = rng.random() * self.cumulative[-1]
        return self.words[bisect.bisect_left(self.cumulative, point)]


def message_length(rng):
    return max(1, min(300, int(rng.lognormvariate(2.2, 0.8))))


def zipf_shares(rng, count):
    weights = [1 / (rank + 1) ** 0.9 for rank in range(count)]
    rng.shuffle(weights)
    total = sum(weights)
    return [weight / total for weight in weights]


def build_users(count):
    users = []
    for index in range(count):
        first = FIRST_NAMES[index % len(FIRST_NAMES)]
        last = LAST_NAMES[(index * 7) % len(LAST_NAMES)]
        handle = f"{first}.{last}".lower()
        users.append(
            {
                "id": f"U{index + 1:07d}",
                "name": handle,
                "deleted": False,
                "is_bot": False,
                "profile": {
                    "email": f"{handle}{index + 1}@scale.test",
                    "real_name": f"{first} {last}",
                    "display_name": first,
                },
            }
        )
    return users


def build_conversations(rng, args, user_ids):
    private_count = round(args.channels * args.private_share)
    public, private, dms, mpims = [], [], [], []
    for index in range(args.channels):
        is_private = index >= args.channels - private_count
        size = len(user_ids) if index == 0 else rng.randint(4, max(4, len(user_ids) // 2))
        members = sorted(rng.sample(user_ids, size))
        conversation = {
            "id": f"{'G' if is_private else 'C'}{index + 1:08d}",
            "name": "general" if index == 0 else f"{'priv' if is_private else 'team'}-{index:03d}",
            "is_archived": index % 37 == 36,
            "members": members,
            "topic": {"value": ""},
            "purpose": {"value": f"Synthetic channel {index}"},
        }
        (private if is_private else public).append(conversation)
    pairs = rng.sample(list(itertools.combinations(user_ids, 2)), args.dms)
    for index, pair in enumerate(pairs):
        dms.append({"id": f"D{index + 1:08d}", "members": list(pair)})
    for index in range(args.group_dms):
        members = sorted(rng.sample(user_ids, rng.randint(3, 6)))
        mpims.append(
            {
                "id": f"G9{index + 1:07d}",
                "name": "mpdm-" + "--".join(members) + "-1",
                "members": members,
            }
        )
    return public, private, dms, mpims


def slack_ts(seconds, used):
    micros = int(seconds * 1_000_000)
    while micros in used:
        micros += 1
    used.add(micros)
    return f"{micros // 1_000_000}.{micros % 1_000_000:06d}"


def working_time(rng, start, days):
    while True:
        day = rng.randrange(days)
        moment = start + dt.timedelta(days=day)
        if moment.weekday() >= 5 and rng.random() < 0.85:
            continue
        hour = min(23.99, max(0.0, rng.gauss(13, 3)))
        return (moment + dt.timedelta(hours=hour)).timestamp()


def text_for(rng, vocabulary, members, args):
    words = [vocabulary.word(rng) for _ in range(message_length(rng))]
    if rng.random() < args.mention_share:
        words.insert(rng.randrange(len(words) + 1), f"<@{rng.choice(members)}>")
    text = " ".join(words)
    return text[0].upper() + text[1:]


def message(rng, vocabulary, members, seconds, used, args):
    body = {
        "type": "message",
        "user": rng.choice(members),
        "text": text_for(rng, vocabulary, members, args),
        "ts": slack_ts(seconds, used),
    }
    if rng.random() < args.reaction_share:
        body["reactions"] = [
            {"name": name, "users": rng.sample(members, rng.randint(1, min(4, len(members)))), "count": 0}
            for name in rng.sample(EMOJI, rng.randint(1, 3))
        ]
        for reaction in body["reactions"]:
            reaction["count"] = len(reaction["users"])
    if rng.random() < args.file_share:
        name, mimetype = rng.choice(MIMETYPES)
        body["files"] = [
            {
                "id": f"F{rng.getrandbits(40):012X}",
                "name": name,
                "mimetype": mimetype,
                "mode": "hosted",
                "url_private_download": f"https://files.slack.com/files-pri/T0/{name}",
            }
        ]
    return body


def conversation_messages(rng, vocabulary, conversation, count, start, args):
    members = conversation["members"]
    used = set()
    produced = []
    while len(produced) < count:
        parent_seconds = working_time(rng, start, args.days)
        parent = message(rng, vocabulary, members, parent_seconds, used, args)
        produced.append(parent)
        if rng.random() >= args.thread_share or len(produced) >= count:
            continue
        replies = min(count - len(produced), 1 + int(rng.expovariate(1 / 2.5)))
        parent["thread_ts"] = parent["ts"]
        parent["reply_count"] = replies
        reply_seconds = parent_seconds
        for _ in range(replies):
            reply_seconds += rng.expovariate(1 / 1800)
            reply = message(rng, vocabulary, members, reply_seconds, used, args)
            reply["thread_ts"] = parent["ts"]
            produced.append(reply)
    return produced


def by_day(messages):
    days = {}
    for item in messages:
        stamp = dt.datetime.fromtimestamp(float(item["ts"]), dt.timezone.utc).date().isoformat()
        days.setdefault(stamp, []).append(item)
    for day in days.values():
        day.sort(key=lambda item: float(item["ts"]))
    return days


def main():
    args = parse_args()
    rng = random.Random(args.seed)
    vocabulary = Vocabulary(rng)
    users = build_users(args.users)
    user_ids = [user["id"] for user in users]
    public, private, dms, mpims = build_conversations(rng, args, user_ids)

    end = dt.datetime.fromisoformat(args.end).replace(tzinfo=dt.timezone.utc)
    start = end - dt.timedelta(days=args.days - 1)
    conversations = public + private + dms + mpims
    folders = [item.get("name") or item["id"] for item in conversations]
    shares = zipf_shares(rng, len(conversations))
    counts = [math.floor(share * args.messages) for share in shares]
    for index in range(args.messages - sum(counts)):
        counts[index % len(counts)] += 1

    totals = {"messages": 0, "threads": 0, "replies": 0, "reactions": 0, "files": 0}
    with zipfile.ZipFile(args.out, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        archive.writestr("users.json", json.dumps(users))
        archive.writestr("channels.json", json.dumps(public))
        archive.writestr("groups.json", json.dumps(private))
        archive.writestr("dms.json", json.dumps(dms))
        archive.writestr("mpims.json", json.dumps(mpims))
        for conversation, folder, count in zip(conversations, folders, counts):
            produced = conversation_messages(rng, vocabulary, conversation, count, start, args)
            for item in produced:
                totals["messages"] += 1
                totals["threads"] += "reply_count" in item
                totals["replies"] += "thread_ts" in item and item["thread_ts"] != item["ts"]
                totals["reactions"] += len(item.get("reactions", []))
                totals["files"] += len(item.get("files", []))
            for day, items in sorted(by_day(produced).items()):
                archive.writestr(f"{folder}/{day}.json", json.dumps(items))

    print(
        json.dumps(
            {
                "out": args.out,
                "users": len(users),
                "public_channels": len(public),
                "private_channels": len(private),
                "dms": len(dms),
                "group_dms": len(mpims),
                **totals,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()

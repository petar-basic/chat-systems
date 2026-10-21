#!/usr/bin/env python3
import argparse
import json
import os
import statistics
import time
import urllib.parse
import urllib.request


def parse_args():
    parser = argparse.ArgumentParser(
        description=(
            "Times search, channel history and the unread counters against a running "
            "instance, as one signed-in user, and prints p50/p95/max per scenario."
        )
    )
    parser.add_argument("--api", default="http://localhost:3000/api")
    parser.add_argument("--email", default=os.environ.get("SCALE_EMAIL"))
    parser.add_argument("--password", default=os.environ.get("SCALE_PASSWORD"))
    parser.add_argument("--workspace", required=True, help="workspace uuid")
    parser.add_argument("--channel", required=True, help="channel uuid to page history in")
    parser.add_argument("--runs", type=int, default=20)
    parser.add_argument("--rare-word", required=True, help="a word that matches a few hundred messages")
    parser.add_argument("--common-word", default="deploy")
    parser.add_argument("--very-common-word", default="the", help="a word in most messages")
    parser.add_argument("--substring", default="ploy")
    parser.add_argument("--json", help="also write the results here")
    return parser.parse_args()


class Client:
    def __init__(self, api, token=None):
        self.api = api
        self.token = token

    def call(self, method, path, body=None):
        data = json.dumps(body).encode() if body is not None else None
        request = urllib.request.Request(self.api + path, data=data, method=method)
        request.add_header("Content-Type", "application/json")
        if self.token:
            request.add_header("Authorization", f"Bearer {self.token}")
        started = time.perf_counter()
        with urllib.request.urlopen(request, timeout=120) as response:
            payload = json.loads(response.read() or b"null")
        return payload, (time.perf_counter() - started) * 1000

    def get(self, path, **query):
        query = {key: value for key, value in query.items() if value is not None}
        suffix = "?" + urllib.parse.urlencode(query) if query else ""
        return self.call("GET", path + suffix)


def rows(payload):
    return payload["data"] if isinstance(payload, dict) and "data" in payload else payload


def summarise(samples):
    ordered = sorted(samples)
    p95 = ordered[min(len(ordered) - 1, round(0.95 * (len(ordered) - 1)))]
    return {
        "p50": round(statistics.median(ordered), 1),
        "p95": round(p95, 1),
        "max": round(ordered[-1], 1),
    }


def timed(client, runs, path, **query):
    client.get(path, **query)
    samples, last = [], None
    for _ in range(runs):
        last, elapsed = client.get(path, **query)
        samples.append(elapsed)
    return summarise(samples), last


def history_cursor(client, channel, pages):
    cursor = None
    for _ in range(pages):
        page, _ = client.get(f"/channels/{channel}/messages", limit=50, cursor=cursor)
        items = rows(page)
        if not items:
            break
        cursor = min(items, key=lambda item: item["created_at"])["id"]
    return cursor


def main():
    args = parse_args()
    login, _ = Client(args.api).call("POST", "/auth/login", {"email": args.email, "password": args.password})
    client = Client(args.api, login["access_token"])
    ws, channel, runs = args.workspace, args.channel, args.runs

    scenarios = {}
    for label, query, offset in [
        ("search: rare word", args.rare_word, 0),
        ("search: common word", args.common_word, 0),
        ("search: common word, page 11", args.common_word, 200),
        ("search: very common word", args.very_common_word, 0),
        ("search: substring", args.substring, 0),
        ("search: two words", f"{args.common_word} review", 0),
    ]:
        result, payload = timed(client, runs, "/search", q=query, workspace_id=ws, limit=20, offset=offset)
        scenarios[label] = {**result, "rows": len(rows(payload))}

    result, _ = timed(client, runs, f"/channels/{channel}/messages", limit=50)
    scenarios["history: newest page"] = result
    deep = history_cursor(client, channel, 100)
    result, _ = timed(client, runs, f"/channels/{channel}/messages", limit=50, cursor=deep)
    scenarios["history: page 100"] = result

    result, _ = timed(client, runs, f"/workspaces/{ws}/channels/unread")
    scenarios["unread: per channel"] = result
    result, _ = timed(client, runs, f"/workspaces/{ws}/notifications/unread-count")
    scenarios["unread: notification count"] = result
    result, _ = timed(client, runs, f"/workspaces/{ws}/channels")
    scenarios["sidebar: channel list"] = result

    width = max(len(label) for label in scenarios)
    print(f"{'scenario'.ljust(width)}  {'p50 ms':>8}  {'p95 ms':>8}  {'max ms':>8}  rows")
    for label, numbers in scenarios.items():
        print(
            f"{label.ljust(width)}  {numbers['p50']:>8}  {numbers['p95']:>8}  {numbers['max']:>8}  "
            f"{numbers.get('rows', '')}"
        )
    if args.json:
        with open(args.json, "w") as handle:
            json.dump(scenarios, handle, indent=2)


if __name__ == "__main__":
    main()

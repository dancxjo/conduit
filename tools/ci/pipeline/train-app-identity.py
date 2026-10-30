"""Bounded App-JWT identity reads; no token arguments, redirects, or mutations."""
import json
import os
import re
import sys
import urllib.request


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def main():
    endpoint = sys.argv[1] if len(sys.argv) == 2 else ''
    if not re.fullmatch(r'app(?:/installations/[1-9][0-9]*)?', endpoint):
        raise ValueError('endpoint')
    token = os.environ['GH_TOKEN']
    if not re.fullmatch(r'[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+', token):
        raise ValueError('JWT')
    request = urllib.request.Request('https://api.github.com/' + endpoint, headers={
        'Authorization': 'Bearer ' + token, 'Accept': 'application/vnd.github+json',
        'User-Agent': 'conduit-train-controller'})
    with urllib.request.build_opener(NoRedirect).open(request, timeout=60) as response:
        data = response.read(2 * 1024 * 1024 + 1)
        if len(data) > 2 * 1024 * 1024:
            raise ValueError('response bound')
        json.loads(data)
        sys.stdout.buffer.write(data)


if __name__ == '__main__':
    try:
        main()
    except Exception:
        sys.stderr.write('App identity observation refused\n')
        sys.exit(1)

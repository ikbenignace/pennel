#!/usr/bin/env python3
"""Require successful selected checks while accepting intentional job skips."""

import json
import os
import sys


def failures(needs):
    errors = []
    changes = needs.get('changes', {})
    if changes.get('result') != 'success':
        errors.append('Change selection did not succeed')
    outputs = changes.get('outputs', {})
    for flag in ('core', 'legacy', 'platform'):
        if outputs.get(flag) not in ('true', 'false'):
            errors.append(f'Missing or invalid {flag} selection')
    if outputs.get('core') == 'true' and outputs.get('legacy') != 'true':
        errors.append('Core checks require the macOS/core build job')
    required = {
        'build-and-test': outputs.get('legacy') == 'true',
        'platform': outputs.get('platform') == 'true' and outputs.get('legacy') != 'true',
    }
    for job, selected in required.items():
        result = needs.get(job, {}).get('result')
        allowed = ('success',) if selected else ('success', 'skipped')
        if result not in allowed:
            errors.append(f'{job}: {result or "missing result"}')
    return errors


def main():
    try:
        needs = json.loads(os.environ['CI_NEEDS_JSON'])
        if not isinstance(needs, dict):
            raise ValueError('Expected a jobs object')
        errors = failures(needs)
    except (KeyError, ValueError, TypeError, AttributeError) as error:
        print(f'Invalid check results: {error}', file=sys.stderr)
        return 1
    for error in errors:
        print(error, file=sys.stderr)
    if not errors:
        print('All selected macOS/core checks passed')
    return int(bool(errors))


if __name__ == '__main__':
    sys.exit(main())

#!/usr/bin/env python3
"""UI-A8: cross-check boundary map vs workspace members."""
import json, re, sys

d = json.load(open('docs/licensing-boundary.json'))
map_names = [c['crate'] for c in d['crates']]

txt = open('Cargo.toml').read()
member_strs = re.findall(r'"crates/([a-z0-9-]+)"', txt)
members = sorted(set(member_strs))

print('workspace member count:', len(members))
print('boundary map entries   :', len(map_names))
print('in workspace, not map :', sorted(set(members) - set(map_names)))
print('in map, not workspace :', sorted(set(map_names) - set(members)))
print('count field           :', d['count'])
print('license_map_total     :', d['license_map_total'])

// spec/requirements.json, read once, with the conventions every reader shares:
// the ID classes, the quality levels, and how a requirement's class is named.

import { readJson } from './common.mjs';

export const REGISTRY = 'spec/requirements.json';
export const LEVELS = ['QL-A', 'QL-B', 'QL-C', 'QL-D'];
export const CLASS_RE = /^AR-(SYS|HLR|LLR|PERF)-\d{3}$/;

export const requirements = () => readJson(REGISTRY).requirements;
export const classOf = (id) => id.match(CLASS_RE)?.[1] ?? null;

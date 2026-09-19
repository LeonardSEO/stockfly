// The JSON Schema subset used by release-manifest.schema.json. Unsupported
// keywords fail closed so a future schema change cannot silently skip validation.
import { isDeepStrictEqual } from 'node:util';
const keywords = new Set(['$schema', 'title', 'type', 'required', 'properties', 'items', 'minItems', 'maxItems', 'minLength', 'pattern', 'minimum', 'const', 'enum']);
const types = {
  object: value => value !== null && typeof value === 'object' && !Array.isArray(value),
  array: Array.isArray,
  integer: Number.isInteger,
  string: value => typeof value === 'string',
};
function checkSchema(schema) {
  for (const key of Object.keys(schema)) {
    if (!keywords.has(key)) throw Error(`Unsupported release schema keyword: ${key}`);
  }
  if (schema.type !== undefined && !Object.hasOwn(types, schema.type)) throw Error(`Unsupported release schema type: ${schema.type}`);
  for (const child of Object.values(schema.properties ?? {})) checkSchema(child);
  if (schema.items) checkSchema(schema.items);
}
function checkValue(value, schema, location) {
  const fail = rule => { throw Error(`Release manifest ${location}: violates ${rule}`); };
  if (schema.type !== undefined && !types[schema.type](value)) fail(`type ${schema.type}`);
  if (Object.hasOwn(schema, 'const') && !isDeepStrictEqual(value, schema.const)) fail('const');
  if (schema.enum && !schema.enum.some(option => isDeepStrictEqual(value, option))) fail('enum');
  if (typeof value === 'string') {
    if (schema.minLength !== undefined && [...value].length < schema.minLength) fail('minLength');
    if (schema.pattern !== undefined && !new RegExp(schema.pattern, 'u').test(value)) fail('pattern');
  }
  if (typeof value === 'number' && schema.minimum !== undefined && value < schema.minimum) fail('minimum');
  if (Array.isArray(value)) {
    if (schema.minItems !== undefined && value.length < schema.minItems) fail('minItems');
    if (schema.maxItems !== undefined && value.length > schema.maxItems) fail('maxItems');
    if (schema.items) value.forEach((item, i) => checkValue(item, schema.items, `${location}[${i}]`));
  }
  if (types.object(value)) {
    for (const required of schema.required ?? []) if (!Object.hasOwn(value, required)) fail(`required ${required}`);
    for (const [key, child] of Object.entries(schema.properties ?? {})) {
      if (Object.hasOwn(value, key)) checkValue(value[key], child, `${location}.${key}`);
    }
  }
}
export function validateReleaseManifest(manifest, schema) {
  checkSchema(schema);
  checkValue(manifest, schema, '$');
}

/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';
import { buildSections, collectFetchProperties } from '../src/components/forms/formLogic';
import { resolveView } from '../src/lib/schemaResolver';
import type { Schema } from '../src/types/schema';

const schema: Schema = JSON.parse(gunzipSync(readFileSync('../resources/schema/schema.json.gz')).toString());
const STEP_UP_FIELDS = ['currentSecret', 'otpCode'];

describe('credential creation requires identity confirmation', () => {
    for (const objectName of ['x:AppPassword', 'x:ApiKey']) {
        const fieldNames = (mode: 'create' | 'edit') =>
            buildSections(schema.forms[objectName], schema.fields[objectName], mode).flatMap((section) =>
                section.fields.map((entry) => entry.formField.name),
            );

        it(`${objectName} asks for the current password only when creating`, () => {
            expect(fieldNames('create')).toEqual(expect.arrayContaining(STEP_UP_FIELDS));
            for (const name of STEP_UP_FIELDS) expect(fieldNames('edit')).not.toContain(name);
        });

        it(`${objectName} never fetches the verification inputs back`, () => {
            const view = resolveView(schema, objectName);
            expect(view).not.toBeNull();
            const properties = collectFetchProperties(schema, view!, objectName);
            for (const name of STEP_UP_FIELDS) expect(properties).not.toContain(name);
        });

        it(`${objectName} treats the current password as a secret`, () => {
            expect(schema.fields[objectName].properties.currentSecret.type).toMatchObject({
                type: 'string',
                format: 'secret',
            });
        });
    }
});

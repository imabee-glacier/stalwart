// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Fingerprint, KeyRound, Loader2, Pencil, Plus, ShieldCheck, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from '@/components/ui/dialog';
import { apiFetchJson } from '@/services/api';
import { errorMessage } from '@/lib/jmapErrors';
import { useAuthStore } from '@/stores/authStore';
import { usePermissionCheckers } from '@/hooks/usePermissions';
import {
    createPasskey,
    authenticateWithPasskey,
    passkeyError,
    passkeysSupported,
    type CeremonyOptions,
} from './webauthn';

interface Passkey {
    id: string;
    name: string;
    createdAt: string;
    lastUsedAt: string | null;
    currentOrigin: boolean;
}
type Editing = { type: 'add' } | { type: 'rename' | 'remove'; key: Passkey };
const endpoint = '/api/account/passkeys';
const post = <T,>(path: string, body: unknown) =>
    apiFetchJson<T>(`${endpoint}/${path}`, {
        method: 'POST',
        body: JSON.stringify(body),
        headers: { 'Content-Type': 'application/json' },
    });

export function PasskeysPage() {
    const { t } = useTranslation();
    const queryClient = useQueryClient();
    const navigate = useNavigate();
    const owner = useAuthStore((s) => s.primaryAccountId);
    const { hasPermission } = usePermissionCheckers();
    const canManage = hasPermission('sysAccountPasswordUpdate');
    const queryKey = ['account-passkeys', owner];
    const query = useQuery({ queryKey, queryFn: () => apiFetchJson<{ list: Passkey[] }>(endpoint) });
    const [editing, setEditing] = useState<Editing | null>(null);
    const [name, setName] = useState('');
    const [password, setPassword] = useState('');
    const [mfa, setMfa] = useState('');
    const [passwordVerification, setPasswordVerification] = useState(false);
    const [pending, setPending] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [notice, setNotice] = useState<string | null>(null);
    const abort = useRef<AbortController | null>(null);
    useEffect(() => () => abort.current?.abort(), []);
    const supported = passkeysSupported();
    const availableKey = query.data?.list.some((key) => key.currentOrigin) ?? false;
    const usePassword = passwordVerification || !availableKey || !supported;
    function close() {
        abort.current?.abort();
        setEditing(null);
        setPassword('');
        setMfa('');
        setError(null);
    }
    function open(value: Editing) {
        setEditing(value);
        setName(value.type === 'add' ? '' : value.key.name);
        setPassword('');
        setMfa('');
        setError(null);
        setPasswordVerification(false);
        setNotice(null);
    }
    async function submit(event: React.FormEvent) {
        event.preventDefault();
        if (!editing || pending) return;
        setPending(true);
        setError(null);
        abort.current = new AbortController();
        try {
            if (editing.type === 'rename') {
                await post('rename', { credentialId: editing.key.id, name });
            } else {
                let verification: { password: string; mfaToken?: string } | { authorization: string };
                if (usePassword) verification = { password, mfaToken: mfa || undefined };
                else {
                    const challenge = await post<CeremonyOptions>('verify/begin', {});
                    const credential = await authenticateWithPasskey(challenge.options, abort.current.signal);
                    verification = await post<{ authorization: string }>('verify/finish', {
                        challengeId: challenge.challengeId,
                        credential,
                    });
                }
                if (editing.type === 'add') {
                    const challenge = await post<CeremonyOptions>('register/begin', { ...verification, name });
                    const credential = await createPasskey(challenge.options, abort.current.signal);
                    await post('register/finish', { challengeId: challenge.challengeId, credential });
                    setNotice(t('passkeys.added', 'Passkey added. You can use it the next time you sign in.'));
                } else {
                    await post('remove', { ...verification, credentialId: editing.key.id });
                    useAuthStore.getState().logout();
                    queryClient.clear();
                    navigate('/login');
                    return;
                }
            }
            close();
            await queryClient.invalidateQueries({ queryKey });
        } catch (err) {
            if (!abort.current.signal.aborted)
                setError(
                    passkeyError(err) ??
                        errorMessage(err, t('passkeys.failed', 'Could not update passkeys. Please try again.')),
                );
        } finally {
            setPending(false);
            setPassword('');
            setMfa('');
        }
    }
    const formatDate = (value: string) => new Date(value).toLocaleDateString(undefined, { dateStyle: 'medium' });
    return (
        <div className="mx-auto w-full max-w-4xl space-y-6">
            <div className="flex flex-wrap items-start justify-between gap-4">
                <div>
                    <h1 className="page-title">{t('passkeys.title', 'Passkeys')}</h1>
                    <p className="mt-1 text-sm text-muted-foreground">
                        {t('passkeys.subtitle', 'Sign in with your screen lock, fingerprint, or a security key.')}
                    </p>
                </div>
                {canManage && (
                    <Button
                        onClick={() => open({ type: 'add' })}
                        disabled={!supported || query.isPending || query.isError}
                    >
                        <Plus className="mr-2 h-4 w-4" />
                        {t('passkeys.add', 'Add passkey')}
                    </Button>
                )}
            </div>
            <div className="flex gap-3 rounded-2xl bg-muted p-4 text-sm text-muted-foreground">
                <ShieldCheck className="mt-0.5 h-5 w-5 shrink-0" />
                <p>
                    {t(
                        'passkeys.security',
                        'Your private key stays with your device or password manager. Stalwart stores only the public credential used to verify sign-ins.',
                    )}
                </p>
            </div>
            {!supported && (
                <p role="status" className="rounded-xl bg-muted p-4 text-sm">
                    {t('passkeys.unsupported', 'Use a supported browser over HTTPS to add or use passkeys.')}
                </p>
            )}
            {notice && (
                <p role="status" className="rounded-xl bg-highlight p-4 text-sm">
                    {notice}
                </p>
            )}
            {query.isPending ? (
                <div role="status" className="flex justify-center p-12">
                    <Loader2 className="h-5 w-5 animate-spin" aria-label={t('common.loading', 'Loading')} />
                </div>
            ) : query.isError ? (
                <div role="alert" className="rounded-xl bg-destructive-soft p-4 text-sm text-destructive">
                    {errorMessage(
                        query.error,
                        t(
                            'passkeys.loadFailed',
                            'Could not load passkeys. Check that you are using the configured server address.',
                        ),
                    )}
                    <Button variant="ghost" onClick={() => query.refetch()}>
                        {t('common.retry', 'Try again')}
                    </Button>
                </div>
            ) : query.data?.list.length === 0 ? (
                <div className="tile flex flex-col items-center p-12 text-center">
                    <Fingerprint className="mb-4 h-10 w-10 text-dim" />
                    <h2 className="section-title">{t('passkeys.empty', 'No passkeys yet')}</h2>
                    <p className="mt-2 max-w-sm text-sm text-muted-foreground">
                        {t(
                            'passkeys.emptyHint',
                            'Add a passkey for a quick, phishing-resistant way to sign in without entering your password.',
                        )}
                    </p>
                </div>
            ) : (
                <ul className="tile divide-y divide-border" aria-label={t('passkeys.list', 'Your passkeys')}>
                    {query.data?.list.map((key) => (
                        <li key={key.id} className="flex items-center gap-4 p-5">
                            <div className="rounded-xl bg-muted p-3">
                                <KeyRound className="h-5 w-5" />
                            </div>
                            <div className="min-w-0 flex-1">
                                <h2 className="truncate text-sm font-semibold">{key.name}</h2>
                                <p className="mt-1 text-xs text-muted-foreground">
                                    {t('passkeys.created', 'Added {{date}}', { date: formatDate(key.createdAt) })}
                                    {' · '}
                                    {key.lastUsedAt
                                        ? t('passkeys.lastUsed', 'Last used {{date}}', {
                                              date: formatDate(key.lastUsedAt),
                                          })
                                        : t('passkeys.notUsed', 'Not used yet')}
                                </p>
                                {!key.currentOrigin && (
                                    <p className="mt-1 text-xs text-dim">
                                        {t('passkeys.otherAddress', 'Registered at another server address')}
                                    </p>
                                )}
                            </div>
                            {canManage && (
                                <>
                                    <Button
                                        variant="ghost"
                                        size="icon"
                                        aria-label={t('passkeys.renameKey', 'Rename {{name}}', { name: key.name })}
                                        onClick={() => open({ type: 'rename', key })}
                                    >
                                        <Pencil className="h-4 w-4" />
                                    </Button>
                                    <Button
                                        variant="ghost"
                                        size="icon"
                                        aria-label={t('passkeys.removeKey', 'Remove {{name}}', { name: key.name })}
                                        onClick={() => open({ type: 'remove', key })}
                                    >
                                        <Trash2 className="h-4 w-4 text-destructive" />
                                    </Button>
                                </>
                            )}
                        </li>
                    ))}
                </ul>
            )}
            <Dialog
                open={editing !== null}
                onOpenChange={(open) => {
                    if (!open) close();
                }}
            >
                <DialogContent>
                    <form onSubmit={submit}>
                        <DialogHeader>
                            <DialogTitle>
                                {editing?.type === 'remove'
                                    ? t('passkeys.remove', 'Remove passkey')
                                    : editing?.type === 'rename'
                                      ? t('passkeys.rename', 'Rename passkey')
                                      : t('passkeys.add', 'Add passkey')}
                            </DialogTitle>
                            <DialogDescription>
                                {editing?.type === 'remove'
                                    ? t(
                                          'passkeys.removeHint',
                                          'Verify your identity to remove this passkey. You will need to sign in again in the web interface.',
                                      )
                                    : editing?.type === 'rename'
                                      ? t('passkeys.renameHint', 'Choose a name that helps you recognize this passkey.')
                                      : t(
                                            'passkeys.addHint',
                                            'Verify your identity, then follow your browser’s instructions to create a passkey.',
                                        )}
                            </DialogDescription>
                        </DialogHeader>
                        <div className="space-y-4 py-5">
                            {editing?.type !== 'remove' && (
                                <div className="space-y-2">
                                    <Label htmlFor="passkey-name">{t('passkeys.name', 'Passkey name')}</Label>
                                    <Input
                                        id="passkey-name"
                                        value={name}
                                        onChange={(e) => setName(e.target.value)}
                                        placeholder={t('passkeys.namePlaceholder', 'For example, Work laptop')}
                                        maxLength={100}
                                        required
                                        autoFocus
                                        disabled={pending}
                                    />
                                </div>
                            )}
                            {editing?.type !== 'rename' &&
                                (usePassword ? (
                                    <>
                                        <div className="space-y-2">
                                            <Label htmlFor="passkey-password">
                                                {t('passkeys.password', 'Current password')}
                                            </Label>
                                            <Input
                                                id="passkey-password"
                                                type="password"
                                                autoComplete="current-password"
                                                value={password}
                                                onChange={(e) => setPassword(e.target.value)}
                                                required
                                                disabled={pending}
                                            />
                                        </div>
                                        <div className="space-y-2">
                                            <Label htmlFor="passkey-mfa">
                                                {t('passkeys.code', 'Two-factor code (if enabled)')}
                                            </Label>
                                            <Input
                                                id="passkey-mfa"
                                                autoComplete="one-time-code"
                                                inputMode="numeric"
                                                value={mfa}
                                                onChange={(e) => setMfa(e.target.value)}
                                                disabled={pending}
                                            />
                                        </div>
                                        {availableKey && supported && (
                                            <Button
                                                type="button"
                                                variant="ghost"
                                                onClick={() => setPasswordVerification(false)}
                                                disabled={pending}
                                            >
                                                {t('passkeys.verifyWithKey', 'Verify with an existing passkey')}
                                            </Button>
                                        )}
                                    </>
                                ) : (
                                    <>
                                        <p className="text-sm text-muted-foreground">
                                            {t(
                                                'passkeys.verifyHint',
                                                'Your browser will ask you to verify with an existing passkey first.',
                                            )}
                                        </p>
                                        <Button
                                            type="button"
                                            variant="ghost"
                                            onClick={() => setPasswordVerification(true)}
                                            disabled={pending}
                                        >
                                            {t('passkeys.usePassword', 'Use password instead')}
                                        </Button>
                                    </>
                                ))}
                            {error && (
                                <p role="alert" className="rounded-xl bg-destructive-soft p-3 text-sm text-destructive">
                                    {error}
                                </p>
                            )}
                        </div>
                        <DialogFooter>
                            <Button variant="ghost" type="button" onClick={close}>
                                {t('common.cancel', 'Cancel')}
                            </Button>
                            <Button
                                type="submit"
                                disabled={pending}
                                variant={editing?.type === 'remove' ? 'destructive' : 'default'}
                            >
                                {pending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                                {pending
                                    ? t('passkeys.waiting', 'Waiting for verification…')
                                    : editing?.type === 'remove'
                                      ? t('passkeys.remove', 'Remove passkey')
                                      : editing?.type === 'rename'
                                        ? t('common.save', 'Save')
                                        : t('passkeys.continue', 'Continue')}
                            </Button>
                        </DialogFooter>
                    </form>
                </DialogContent>
            </Dialog>
        </div>
    );
}

// Terminal route (RFC-038 §10.4).
import type { JSX } from 'react';
//
// Renders the Terminal component full-bleed (AppLayout branches on
// isTerminal to match Chat's `isChat` full-bleed behavior).

import { createFileRoute } from '@tanstack/react-router';
import { Terminal } from '@/components/terminal/Terminal';
import { useTranslation } from 'react-i18next';

export const Route = createFileRoute('/terminal')({
  component: TerminalPage,
});

function TerminalPage(): JSX.Element {
  const { t } = useTranslation();
  return (
    <div className="flex h-full w-full flex-col">
      <title>{t('terminal.title', 'Terminal')}</title>
      <Terminal />
    </div>
  );
}
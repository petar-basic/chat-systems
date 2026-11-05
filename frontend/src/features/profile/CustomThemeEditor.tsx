import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Shuffle } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import { ColourInput } from '@/shared/components/Form/ColourInput';
import { TextInput } from '@/shared/components/Form/TextInput';
import { useThemePreview } from './hooks/useWorkspaceTheme';
import {
  THEME_KEYS,
  parseTheme,
  shareTheme,
  surpriseTheme,
  unreadable,
  type Theme,
  type ThemeKey,
} from './workspaceThemes';

interface Props {
  initial: Theme;
  saving: boolean;
  onSave: (theme: Theme) => void;
  onCancel: () => void;
}

export default function CustomThemeEditor({ initial, saving, onSave, onCancel }: Props) {
  const { t, i18n } = useTranslation('profile');
  const [draft, setDraft] = useState(initial);
  const [line, setLine] = useState('');
  const [importError, setImportError] = useState<string | null>(null);
  const preview = useThemePreview((s) => s.preview);

  useEffect(() => {
    preview(draft);
  }, [draft, preview]);
  useEffect(() => () => preview(null), [preview]);

  const importLine = () => {
    const theme = parseTheme(line);
    if (!theme) {
      setImportError(t('customTheme.importError'));
      return;
    }
    setDraft(theme);
    setLine('');
    setImportError(null);
  };

  const hard = new Intl.ListFormat(i18n.language, { type: 'conjunction' }).format(
    unreadable(draft).map((key: ThemeKey) => t(`customTheme.partsInSentence.${key}`)),
  );

  return (
    <div className="mt-3 rounded-lg border border-line-strong p-3" data-qa="custom-theme">
      <div className="grid grid-cols-2 gap-3">
        {THEME_KEYS.map((key) => (
          <ColourInput
            key={key}
            label={t(`customTheme.parts.${key}`)}
            value={draft[key]}
            onChange={(value) => setDraft((theme) => ({ ...theme, [key]: value }))}
            dataQa={`custom-theme-${key}`}
          />
        ))}
      </div>
      {hard && (
        <p className="mt-2 text-xs text-warning" data-qa="custom-theme-unreadable">
          {t('customTheme.unreadable', { parts: hard })}
        </p>
      )}
      <CopyField
        label={t('customTheme.share')}
        value={shareTheme(draft)}
        valueQa="custom-theme-share"
        copyQa="custom-theme-copy"
      />
      <div className="mt-3 flex items-start gap-2">
        <TextInput
          className="flex-1"
          label={t('customTheme.import')}
          compact
          value={line}
          placeholder="#3f0e40,#1164a3,#2bac76,#cd2553"
          error={importError}
          dataQa="custom-theme-import"
          onChange={(event) => {
            setLine(event.target.value);
            setImportError(null);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              importLine();
            }
          }}
        />
        <Button size="sm" className="mt-7" disabled={!line.trim()} onClick={importLine}>
          {t('customTheme.importButton')}
        </Button>
      </div>
      <div className="mt-4 flex items-center justify-between gap-2">
        <Button
          variant="ghost"
          size="sm"
          icon={<Shuffle className="w-3.5 h-3.5" />}
          onClick={() => setDraft(surpriseTheme())}
          data-qa="custom-theme-surprise"
        >
          {t('customTheme.surprise')}
        </Button>
        <div className="flex gap-2">
          <Button variant="ghost" size="sm" onClick={onCancel}>
            {t('customTheme.cancel')}
          </Button>
          <Button
            variant="primary"
            size="sm"
            loading={saving}
            onClick={() => onSave(draft)}
            data-qa="custom-theme-save"
          >
            {t('customTheme.save')}
          </Button>
        </div>
      </div>
    </div>
  );
}

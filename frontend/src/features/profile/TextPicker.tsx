import { useTranslation } from 'react-i18next';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { useTextStore, type TextSize, type Typeface } from '@/stores/text';
import { useUpdatePreferences } from './hooks/usePreferences';
import SavedEverywhere from './SavedEverywhere';

export default function TextPicker() {
  const { t } = useTranslation('profile');
  const size = useTextStore((s) => s.size);
  const typeface = useTextStore((s) => s.typeface);
  const setSize = useTextStore((s) => s.setSize);
  const setTypeface = useTextStore((s) => s.setTypeface);
  const update = useUpdatePreferences();

  const pickSize = (next: TextSize) => {
    setSize(next);
    update.mutate({ text_size: next });
  };
  const pickTypeface = (next: Typeface) => {
    setTypeface(next);
    update.mutate({ typeface: next });
  };

  return (
    <div className="mt-6 space-y-6" data-qa="text-picker">
      <RadioCards<TextSize>
        label={t('text.size')}
        value={size}
        onChange={pickSize}
        options={[
          { value: '90', label: t('text.sizes.smaller'), description: '90 %', dataQa: 'text-size-90' },
          { value: '100', label: t('text.sizes.default'), description: '100 %', dataQa: 'text-size-100' },
          { value: '112', label: t('text.sizes.larger'), description: '112 %', dataQa: 'text-size-112' },
          { value: '125', label: t('text.sizes.largest'), description: '125 %', dataQa: 'text-size-125' },
        ]}
      />
      <RadioCards<Typeface>
        label={t('text.typeface')}
        value={typeface}
        onChange={pickTypeface}
        options={[
          {
            value: 'system',
            label: t('text.typefaces.system.label'),
            description: t('text.typefaces.system.description'),
            dataQa: 'typeface-system',
          },
          {
            value: 'inter',
            label: 'Inter',
            description: t('text.typefaces.inter.description'),
            dataQa: 'typeface-inter',
          },
          {
            value: 'atkinson_hyperlegible',
            label: 'Atkinson Hyperlegible',
            description: t('text.typefaces.atkinson.description'),
            dataQa: 'typeface-atkinson_hyperlegible',
          },
          {
            value: 'open_dyslexic',
            label: 'OpenDyslexic',
            description: t('text.typefaces.openDyslexic.description'),
            dataQa: 'typeface-open_dyslexic',
          },
        ]}
      />
      <p className="text-xs text-muted">{t('text.code')}</p>
      <SavedEverywhere failed={update.isError} />
    </div>
  );
}

import { RadioCards } from '@/shared/components/Form/RadioCards';
import { useTextStore, type TextSize, type Typeface } from '@/stores/text';
import { useUpdatePreferences } from './hooks/usePreferences';
import SavedEverywhere from './SavedEverywhere';

export default function TextPicker() {
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
        label="Text size"
        value={size}
        onChange={pickSize}
        options={[
          { value: '90', label: 'Smaller', description: '90 %', dataQa: 'text-size-90' },
          { value: '100', label: 'Default', description: '100 %', dataQa: 'text-size-100' },
          { value: '112', label: 'Larger', description: '112 %', dataQa: 'text-size-112' },
          { value: '125', label: 'Largest', description: '125 %', dataQa: 'text-size-125' },
        ]}
      />
      <RadioCards<Typeface>
        label="Typeface"
        value={typeface}
        onChange={pickTypeface}
        options={[
          {
            value: 'system',
            label: 'System',
            description: 'The font your device uses everywhere else.',
            dataQa: 'typeface-system',
          },
          {
            value: 'inter',
            label: 'Inter',
            description: 'Clear and even, made for screens.',
            dataQa: 'typeface-inter',
          },
          {
            value: 'atkinson_hyperlegible',
            label: 'Atkinson Hyperlegible',
            description: 'Letters that are hard to mistake for one another, for low vision.',
            dataQa: 'typeface-atkinson_hyperlegible',
          },
          {
            value: 'open_dyslexic',
            label: 'OpenDyslexic',
            description: 'Weighted letters that some people with dyslexia find easier to read.',
            dataQa: 'typeface-open_dyslexic',
          },
        ]}
      />
      <p className="text-xs text-muted">Code keeps its monospace font.</p>
      <SavedEverywhere failed={update.isError} />
    </div>
  );
}

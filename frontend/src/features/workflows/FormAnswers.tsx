import RichTextDisplay from '@/components/RichTextDisplay';
import { isLongAnswer, type MessageField } from './answerFields';

export function FormAnswers({ fields }: { fields: MessageField[] }) {
  return (
    <dl className="mt-1 grid max-w-2xl gap-x-6 gap-y-2 sm:grid-cols-2" data-qa="form-answers">
      {fields.map((field, index) => (
        <div key={index} className={isLongAnswer(field.value) ? 'sm:col-span-2' : ''}>
          <dt className="text-xs font-semibold text-muted">{field.label}</dt>
          <dd className="text-sm text-fg">
            {field.value.trim() ? <RichTextDisplay content={field.value} /> : '—'}
          </dd>
        </div>
      ))}
    </dl>
  );
}

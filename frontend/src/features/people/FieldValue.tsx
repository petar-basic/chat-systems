import { useUserCache } from '@/stores/users';
import { useProfilePanel } from '@/stores/profilePanel';
import { displayNameOf } from '@/lib/userHelpers';
import type { ProfileField } from './hooks/useProfileFields';
import { formatFieldDate, optionLabel } from './fieldValues';

interface Props {
  field: ProfileField;
  value: string;
}

export function FieldValue({ field, value }: Props) {
  switch (field.kind) {
    case 'link':
      return (
        <a
          href={value}
          target="_blank"
          rel="noopener noreferrer"
          className="text-accent hover:underline break-all"
          data-qa="profile-field-link"
        >
          {value.replace(/^https?:\/\//, '')}
        </a>
      );
    case 'date':
      return <span>{formatFieldDate(value)}</span>;
    case 'options':
      return <span>{optionLabel(field, value) ?? ''}</span>;
    case 'person':
      return <PersonValue userId={value} />;
    default:
      return <span className="break-words">{value}</span>;
  }
}

function PersonValue({ userId }: { userId: string }) {
  const person = useUserCache((s) => s.getUser(userId));
  const show = useProfilePanel((s) => s.show);
  return (
    <button
      type="button"
      onClick={() => show(userId)}
      className="text-accent hover:underline cursor-pointer text-left"
      data-qa="profile-field-person"
    >
      {displayNameOf(person?.display_name || person?.email)}
    </button>
  );
}

export function FieldList({
  entries,
  dataQa,
  className = '',
}: {
  entries: { field: ProfileField; value: string }[];
  dataQa: string;
  className?: string;
}) {
  if (entries.length === 0) return null;
  return (
    <dl className={`grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs ${className}`} data-qa={dataQa}>
      {entries.map(({ field, value }) => (
        <div key={field.id} className="contents" data-qa="profile-field-row">
          <dt className="text-muted truncate max-w-32">{field.label}</dt>
          <dd className="text-fg-dim min-w-0">
            <FieldValue field={field} value={value} />
          </dd>
        </div>
      ))}
    </dl>
  );
}

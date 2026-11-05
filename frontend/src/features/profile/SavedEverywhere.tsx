import { useTranslation } from 'react-i18next';

export default function SavedEverywhere({ failed }: { failed: boolean }) {
  const { t } = useTranslation('profile');
  return (
    <>
      <p className="text-sm text-muted">{t('savedEverywhere.note')}</p>
      {failed && (
        <p role="alert" className="text-sm text-danger">
          {t('savedEverywhere.failed')}
        </p>
      )}
    </>
  );
}

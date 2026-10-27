import { Camera } from 'lucide-react';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { Button } from '@/shared/components/Button/Button';
import { FilePicker } from '@/shared/components/Form/FilePicker';
import { buttonStyles } from '@/shared/components/Button/buttonStyles';
import { Spinner } from '@/shared/components/Spinner/Spinner';

interface Props {
  userId: string;
  name: string;
  shown: string | null;
  uploading: boolean;
  resetLabel: string | null;
  qaPrefix: string;
  onFile: (file: File) => void;
  onReset: () => void;
}

export function ProfilePhotoField({
  userId,
  name,
  shown,
  uploading,
  resetLabel,
  qaPrefix,
  onFile,
  onReset,
}: Props) {
  return (
    <div className="order-first flex flex-col items-center gap-2 sm:order-none sm:w-40">
      <span className="self-start text-sm font-medium text-fg-dim max-sm:sr-only">Profile photo</span>
      <Avatar userId={userId} name={name} avatarUrl={shown} size="2xl" />
      <FilePicker
        label="Profile photo file"
        accept="image/*"
        onFiles={([file]) => onFile(file)}
        disabled={uploading}
        dataQa={`${qaPrefix}-avatar-upload`}
        className={buttonStyles({ size: 'sm' })}
      >
        {uploading ? <Spinner size="sm" /> : <Camera className="h-3.5 w-3.5" />}
        Upload photo
      </FilePicker>
      {resetLabel && (
        <Button
          size="sm"
          variant="ghost"
          onClick={onReset}
          data-qa={`${qaPrefix}-avatar-remove`}
          className="hover:text-danger"
        >
          {resetLabel}
        </Button>
      )}
    </div>
  );
}

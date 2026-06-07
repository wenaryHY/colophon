import { Select, SelectItem } from '../Select';
import type { MediaCategory } from '../../types';

interface MediaCategorySelectProps {
  categories: MediaCategory[];
  value: string;
  onChange: (value: string) => void;
  label?: string;
  placeholder?: string;
  includeEmpty?: boolean;
  emptyLabel?: string;
  disabled?: boolean;
}

export function MediaCategorySelect({
  categories,
  value,
  onChange,
  label,
  placeholder,
  includeEmpty = true,
  emptyLabel = '无分类',
  disabled = false,
}: MediaCategorySelectProps) {
  return (
    <Select
      label={label}
      value={value}
      disabled={disabled}
      onChange={(e) => onChange(e.target.value)}
    >
      {includeEmpty && <SelectItem value="">{placeholder || emptyLabel}</SelectItem>}
      {categories.map((category) => (
        <SelectItem key={category.id} value={category.slug}>
          {category.name}
        </SelectItem>
      ))}
    </Select>
  );
}

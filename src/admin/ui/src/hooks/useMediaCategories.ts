import { useQuery, useMutation } from '@tanstack/react-query';
import { listMediaCategories, createMediaCategory, updateMediaCategory, deleteMediaCategory } from '../lib/api';
import { getQueryClient } from '../lib/api';
import type { CreateMediaCategoryRequest, UpdateMediaCategoryRequest } from '../types';

const QUERY_KEY = ['mediaCategories'] as const;

export function useMediaCategories() {
  const { data: categories = [], isLoading, error } = useQuery({
    queryKey: QUERY_KEY,
    queryFn: () => listMediaCategories(),
  });

  const createMutation = useMutation({
    mutationFn: (data: CreateMediaCategoryRequest) => createMediaCategory(data),
    onSuccess: () => {
      getQueryClient().invalidateQueries({ queryKey: QUERY_KEY });
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateMediaCategoryRequest }) =>
      updateMediaCategory(id, data),
    onSuccess: () => {
      getQueryClient().invalidateQueries({ queryKey: QUERY_KEY });
    },
  });

  const removeMutation = useMutation({
    mutationFn: (id: string) => deleteMediaCategory(id),
    onSuccess: () => {
      getQueryClient().invalidateQueries({ queryKey: QUERY_KEY });
    },
  });

  return {
    categories,
    loading: isLoading,
    error: error ? String(error) : null,
    fetch: () => getQueryClient().invalidateQueries({ queryKey: QUERY_KEY }),
    create: (data: CreateMediaCategoryRequest) => createMutation.mutateAsync(data),
    update: (id: string, data: UpdateMediaCategoryRequest) => updateMutation.mutateAsync({ id, data }),
    remove: (id: string) => removeMutation.mutateAsync(id),
  };
}

-- Add deployment_mode column to clusters table
-- Deployment modes: 'shared_nothing' (default, storage-compute integrated) or 'shared_data' (storage-compute separated)

ALTER TABLE clusters ADD COLUMN deployment_mode VARCHAR(20) DEFAULT 'shared_nothing' NOT NULL;

-- Add index for deployment_mode for faster filtering
CREATE INDEX IF NOT EXISTS idx_clusters_deployment_mode ON clusters(deployment_mode);

-- Add comment to document the field
-- shared_nothing: Traditional architecture with BE nodes (Backend for both storage and compute)
-- shared_data: Modern architecture with CN nodes (Compute Nodes) and separate object storage (S3/HDFS)

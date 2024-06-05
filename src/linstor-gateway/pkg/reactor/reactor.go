package reactor

import (
	"context"
	"encoding"
	"errors"
	"fmt"
	log "github.com/sirupsen/logrus"
	"strings"
	"time"

	"github.com/LINBIT/golinstor/client"
	"github.com/pelletier/go-toml"
)

const (
	promoterDir       = "/etc/drbd-reactor.d"
	gatewayConfigPath = promoterDir + "/linstor-gateway-%s.toml"
)

// Config is the root configuration for drbd-reactor.
//
// Currently, only supports Promoter plugins.
type Config struct {
	Promoter []PromoterConfig `toml:"promoter,omitempty"`
}

// PromoterMetadata is a custom extension to the drbd-reactor config format.
// It stores fields specific to linstor-gateway.
type PromoterMetadata struct {
	LinstorGatewaySchemaVersion int `toml:"linstor-gateway-schema-version"`
}

// PromoterConfig is the configuration for drbd-reactors "promoter" plugin.
type PromoterConfig struct {
	// Deprecated: ID is no longer required in drbd-reactor v1.2.0 and newer.
	ID        string                            `toml:"id,omitempty"`
	Resources map[string]PromoterResourceConfig `toml:"resources,omitempty"`
	Metadata  PromoterMetadata                  `toml:"metadata,omitempty"`
}

func (p *PromoterConfig) FirstResource() (string, *PromoterResourceConfig) {
	for k, c := range p.Resources {
		return k, &c
	}
	return "", nil
}

// DeployedResources fetches the current state of the resources referenced in the promoter config.
func (p *PromoterConfig) DeployedResources(ctx context.Context, cli *client.Client) (*client.ResourceDefinition, *client.ResourceGroup, []client.VolumeDefinition, []client.ResourceWithVolumes, error) {
	var rscNames []string
	for k := range p.Resources {
		rscNames = append(rscNames, k)
	}

	if len(rscNames) != 1 {
		return nil, nil, nil, nil, errors.New(fmt.Sprintf("expected exactly 1 resource, got %d", len(rscNames)))
	}

	rd, err := cli.ResourceDefinitions.Get(ctx, rscNames[0])
	if err != nil {
		return nil, nil, nil, nil, fmt.Errorf("failed to fetch resource definition: %w", err)
	}

	rg, err := cli.ResourceGroups.Get(ctx, rd.ResourceGroupName)
	if err != nil {
		return nil, nil, nil, nil, fmt.Errorf("failed to fetch resource group: %w", err)
	}

	vds, err := cli.ResourceDefinitions.GetVolumeDefinitions(ctx, rscNames[0])
	if err != nil {
		return nil, nil, nil, nil, fmt.Errorf("failed to fetch volume definition: %w", err)
	}

	resources, err := cli.Resources.GetResourceView(ctx, &client.ListOpts{Resource: rscNames})
	if err != nil {
		return nil, nil, nil, nil, fmt.Errorf("failed to fetch deployed resources: %w", err)
	}

	return &rd, &rg, vds, resources, nil
}

type StartEntry interface {
	encoding.TextMarshaler
	encoding.TextUnmarshaler
}

// PromoterResourceConfig is the configuration of a single promotable resource used by drbd-reactor's promoter.
type PromoterResourceConfig struct {
	Start               []StartEntry `toml:"start,omitempty"`
	Runner              string       `toml:"runner,omitempty"`
	OnDrbdDemoteFailure string       `toml:"on-drbd-demote-failure,omitempty"`
	StopServicesOnExit  bool         `toml:"stop-services-on-exit,omitempty"`
	TargetAs            string       `toml:"target-as,omitempty"`
}

func (c *PromoterResourceConfig) UnmarshalTOML(data interface{}) error {
	d, _ := data.(map[string]interface{})
	if val, ok := d["start"]; ok {
		start, startOk := val.([]interface{})
		if !startOk {
			return fmt.Errorf("could not convert value %v to slice (is type %T)", val, val)
		}
		for _, entry := range start {
			text, ok := entry.(string)
			if !ok {
				return fmt.Errorf("could not convert value %v to string (is type %T)", entry, entry)
			}
			if strings.HasPrefix(text, "ocf:") {
				var a ResourceAgent
				err := a.UnmarshalText([]byte(text))
				if err != nil {
					return fmt.Errorf("invalid start entry: %w", err)
				}
				c.Start = append(c.Start, &a)
			} else {
				var s SystemdService
				err := s.UnmarshalText([]byte(text))
				if err != nil {
					return fmt.Errorf("invalid start entry: %w", err)
				}
				c.Start = append(c.Start, &s)
			}
		}
	}
	if val, ok := d["runner"]; ok {
		c.Runner, ok = val.(string)
		if !ok {
			return fmt.Errorf("could not convert value %v to string (is type %T)", val, val)
		}
	}
	if val, ok := d["on-drbd-demote-failure"]; ok {
		c.OnDrbdDemoteFailure, ok = val.(string)
		if !ok {
			return fmt.Errorf("could not convert value %v to string (is type %T)", val, val)
		}
	}
	if val, ok := d["stop-services-on-exit"]; ok {
		c.StopServicesOnExit, ok = val.(bool)
		if !ok {
			return fmt.Errorf("could not convert value %v to bool (is type %T)", val, val)
		}
	}
	if val, ok := d["target-as"]; ok {
		c.TargetAs, ok = val.(string)
		if !ok {
			return fmt.Errorf("could not convert value %v to string (is type %T)", val, val)
		}
	}
	return nil
}

// EnsureConfig ensures the given config is registered in LINSTOR and up-to-date.
func EnsureConfig(ctx context.Context, cli *client.Client, cfg *PromoterConfig, id string) error {
	buffer := strings.Builder{}
	buffer.WriteString("# Generated by LINSTOR Gateway at " + time.Now().String() + "\n")
	buffer.WriteString("# DO NOT MODIFY!\n")
	encoder := toml.NewEncoder(&buffer).ArraysWithOneElementPerLine(true)

	err := encoder.Encode(&Config{Promoter: []PromoterConfig{*cfg}})
	if err != nil {
		return fmt.Errorf("error encoding promoter config: %w", err)
	}

	path := ConfigPath(id)
	err = cli.Controller.ModifyExternalFile(ctx, path, client.ExternalFile{Path: path, Content: []byte(buffer.String())})
	if err != nil {
		return fmt.Errorf("error setting promoter config in linstor: %w", err)
	}

	return nil
}

// AttachConfig ensures the promoter config is attached to all referenced resources.
func AttachConfig(ctx context.Context, cli *client.Client, cfg *PromoterConfig, path string) error {
	for rd := range cfg.Resources {
		err := cli.ResourceDefinitions.AttachExternalFile(ctx, rd, path)
		if err != nil {
			return fmt.Errorf("error attaching file to resource: %w", err)
		}
	}

	return nil
}

// DetachConfig detaches the promoter config from all resources.
func DetachConfig(ctx context.Context, cli *client.Client, cfg *PromoterConfig, path string) error {
	for rd := range cfg.Resources {
		err := cli.ResourceDefinitions.DetachExternalFile(ctx, rd, path)
		if err != nil {
			return fmt.Errorf("error attaching file to resource: %w", err)
		}
	}

	return nil
}

// filterConfigs takes a list of external files in the LINSTOR cluster and
// extracts all drbd-reactor promoter configuration files that were created by
// LINSTOR Gateway.
func filterConfigs(files []client.ExternalFile) ([]PromoterConfig, []string, error) {
	result := make([]PromoterConfig, 0, len(files))
	paths := make([]string, 0, len(files))

	for _, file := range files {
		var name string
		n, _ := fmt.Sscanf(file.Path, gatewayConfigPath, &name)
		if n == 0 {
			continue
		}

		log.Debugf("found config %s", name)

		cfg := Config{}
		err := toml.Unmarshal(file.Content, &cfg)
		if err != nil {
			return nil, nil, fmt.Errorf("failed to decode promoter config: %w", err)
		}

		result = append(result, cfg.Promoter...)
		paths = append(paths, file.Path)
	}

	return result, paths, nil
}

// ListConfigs fetches all promoter configurations registered with LINSTOR. It
// returns a slice of PromoterConfigs as well as a slice of configuration file
// paths.
func ListConfigs(ctx context.Context, cli *client.Client) ([]PromoterConfig, []string, error) {
	files, err := cli.Controller.GetExternalFiles(ctx, &client.ListOpts{Content: true})
	if err != nil {
		return nil, nil, fmt.Errorf("failed to fetch file list: %w", err)
	}
	return filterConfigs(files)
}

// FindConfig fetches the promoter config with the given id. It returns the
// corresponding PromoterConfig as well as the path of the configuration file.
//
// Returns nil and "" if no config exists.
func FindConfig(ctx context.Context, cli *client.Client, id string) (*PromoterConfig, string, error) {
	cfgPath := ConfigPath(id)
	// TODO: replace by directly looking up the config file once LINSTOR is fixed.
	all, paths, err := ListConfigs(ctx, cli)
	if err != nil {
		return nil, "", err
	}

	for i := range paths {
		if cfgPath == paths[i] {
			return &all[i], paths[i], nil
		}
	}

	return nil, "", nil
}

// DeleteConfig removes the promoter of the given id from LINSTOR.
//
// In case the config did not exist, no error is returned.
func DeleteConfig(ctx context.Context, cli *client.Client, id string) error {
	path := ConfigPath(id)
	err := cli.Controller.DeleteExternalFile(ctx, path)
	if err != nil && err != client.NotFoundError {
		return fmt.Errorf("error removing config file: %w", err)
	}

	return nil
}

// ConfigPath is the file system path of the promoter config with the given type and id once it is deployed.
func ConfigPath(id string) string {
	return fmt.Sprintf(gatewayConfigPath, id)
}
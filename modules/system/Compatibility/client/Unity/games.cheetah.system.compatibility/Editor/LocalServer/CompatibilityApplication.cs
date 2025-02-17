using Cheetah.Platform.Editor.Configuration;
using Cheetah.Platform.Editor.LocalServer.Applications;
using Cheetah.Platform.Editor.LocalServer.Docker;

namespace Cheetah.System.Compatibility.Editor.LocalServer
{
    public class CompatibilityApplication : PlatformApplication
    {
        private const string ServerName = "system-compatibility";
        private readonly string configurationPath = ConfigurationUtils.GetPathToConfigDirectory(ServerName);

        public CompatibilityApplication() : base(ServerName)
        {
            ExternalGrpcServices.Add("cheetah.system.compatibility");
        }

        public override void ConfigureDockerContainerBuilder(DockerContainerBuilder builder)
        {
            base.ConfigureDockerContainerBuilder(builder);
            builder.AddVolumeMappings(configurationPath, "/tmp/");
            builder.AddEnv("CONFIG_FILE", "/tmp/versions.yaml");
        }
    }
}
using System;
using Cheetah.Matches.Realtime.Internal;
using Cheetah.Matches.Realtime.Internal.Plugin.Routers.ByTemplate;
using Cheetah.Matches.Realtime.Types;

namespace Cheetah.Matches.Realtime.DOA.Income.ByTemplate
{
    public class DeletedObjectByTemplateIncomeCommands : IDisposable
    {
        private readonly ushort template;
        private readonly ReferenceList<CheetahObject> stream = new ReferenceList<CheetahObject>();

        private readonly DeleteObjectRouterByTemplate router;
        private readonly CheetahClient client;

        public DeletedObjectByTemplateIncomeCommands(CheetahClient client, ushort template)
        {
            this.client = client;
            this.template = template;

            client.BeforeUpdateHook += BeforeUpdate;

            router = client.GetPlugin<DeleteObjectRouterByTemplate>();
            router.RegisterListener(template, OnDelete);
        }

        private void BeforeUpdate()
        {
            stream.Clear();
        }

        private void OnDelete(CheetahObject cheetahObject)
        {
            ref var item = ref stream.Add();
            item = cheetahObject;
        }

        public ReadonlyReferenceList<CheetahObject> GetStream()
        {
            return stream;
        }

        public void Dispose()
        {
            client.BeforeUpdateHook -= BeforeUpdate;
            router.UnRegisterListener(template, OnDelete);
        }
    }
}
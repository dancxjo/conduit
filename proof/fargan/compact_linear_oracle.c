/* Development oracle only: generic compact operators from the pinned model.
 * No network ordering or product execution is implemented here. */
#include <stdio.h>
#include <stdint.h>
#include "fargan.c"
#include "nnet.h"
int main(int argc, char **argv) {
  FARGANState st;FILE *out;float in[688],result[480];int i,j,test;
  if(argc!=2)return 2;out=fopen(argv[1],"wb");if(!out)return 3;fargan_init(&st);
  const LinearLayer *layers[]={
    &st.model.cond_net_fconv1,&st.model.cond_net_fdense2,&st.model.sig_net_fwc0_conv,
    &st.model.sig_net_fwc0_glu_gate,&st.model.sig_net_gru1_input,&st.model.sig_net_gru1_recurrent,
    &st.model.sig_net_gru2_input,&st.model.sig_net_gru2_recurrent,&st.model.sig_net_gru3_input,
    &st.model.sig_net_gru3_recurrent,&st.model.sig_net_gru1_glu_gate,&st.model.sig_net_gru2_glu_gate,
    &st.model.sig_net_gru3_glu_gate,&st.model.sig_net_skip_glu_gate,&st.model.sig_net_skip_dense,
    &st.model.sig_net_sig_dense_out};
  for(i=0;i<16;i++) {
    const LinearLayer *layer=layers[i];
    if(layer->float_weights!=NULL || layer->weights==NULL || layer->weights_idx!=NULL)return 4;
    for(test=0;test<3;test++) {
      uint32_t header[]={i,test,layer->nb_inputs,layer->nb_outputs};
      for(j=0;j<layer->nb_inputs;j++)in[j]=test==0?0.f:test==1?((j*13%201)-100)/101.f:(j%2?-1.f:1.f);
      compute_linear_c(layer,result,in);fwrite(header,4,4,out);fwrite(in,4,layer->nb_inputs,out);fwrite(result,4,layer->nb_outputs,out);
    }
  }
  fclose(out);return 0;
}

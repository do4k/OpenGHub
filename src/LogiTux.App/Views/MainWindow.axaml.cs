using System;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Platform;
using LogiTux.App.ViewModels;

namespace LogiTux.App.Views;

public partial class MainWindow : Window
{
    private readonly MainWindowViewModel _vm = new();

    public MainWindow()
    {
        Icon = new WindowIcon(AssetLoader.Open(new Uri("avares://LogiTux.App/Assets/app-icon.png")));
        InitializeComponent();
        DataContext = _vm;
        Closed += (_, _) => _vm.DisposeAsync().AsTask().GetAwaiter().GetResult();
    }
}